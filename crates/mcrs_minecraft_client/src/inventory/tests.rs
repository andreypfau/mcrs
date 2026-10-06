use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bytes::Bytes;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_inventory::{MenuLayout, Slot};
use mcrs_minecraft_item::{
    Held, ItemStack, Items, SelectedHotbarSlot, SlotTable, StackRevision, slots,
};
use mcrs_minecraft_network::client::{ReceivedRegistries, ReceivedRegistry, RegistryEntry};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, Instant};
use mcrs_minecraft_protocol::item::{
    ComponentPatch, CustomName, Damage, Enchantments, RawStack, Unbreakable,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundContainerClose, ClientboundContainerSetContent, ClientboundContainerSetSlot,
    ClientboundOpenScreen, ClientboundSetCursorItem, ClientboundSetHeldSlot,
    ClientboundSetPlayerInventory,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Encode, Packet, ProtoStack, RegistryId, VarInt};
use mcrs_minecraft_registry::static_report::shipped_report;
use mcrs_minecraft_registry::{NameTable, RegistryLookup, RegistrySet};
use mcrs_minecraft_world::item::test_corpus;

use super::{ContainerSeqno, InventoryPlugin, OpenMenu, Screen, inventory_index_to_cell};

fn registries() -> &'static RegistrySet {
    static SET: OnceLock<RegistrySet> = OnceLock::new();
    SET.get_or_init(|| {
        shipped_report()
            .clone()
            .with_types(mcrs_minecraft_keys::bindings())
            .unwrap()
    })
}
use crate::player::Player;

const GOLDEN: &str =
    include_str!("../../../mcrs_minecraft_protocol/tests/fixtures/inventory_packets_golden.txt");

fn golden() -> &'static HashMap<&'static str, Vec<u8>> {
    static PACKETS: OnceLock<HashMap<&'static str, Vec<u8>>> = OnceLock::new();
    PACKETS.get_or_init(|| {
        GOLDEN
            .lines()
            .filter(|line| !line.starts_with("id "))
            .filter_map(|line| line.split_once(' '))
            .map(|(name, hex)| {
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                (name, bytes)
            })
            .collect()
    })
}

fn items() -> &'static Items {
    &test_corpus().1
}

struct Client {
    app: App,
    player: Entity,
    connection: Entity,
}

impl Client {
    fn new() -> Self {
        Self::with(registries().clone(), ReceivedRegistries::default())
    }

    fn with(local: RegistrySet, received: ReceivedRegistries) -> Self {
        let mut app = App::new();
        app.insert_resource(items().clone())
            .insert_resource(local)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(InventoryPlugin);
        let player = app
            .world_mut()
            .spawn((
                Player,
                SlotTable::fixed(slots::COUNT),
                SelectedHotbarSlot::default(),
                ContainerSeqno::default(),
            ))
            .id();
        let connection = app
            .world_mut()
            .spawn((ConnectionState::Game, received))
            .id();
        app.finish();
        app.cleanup();
        app.update();
        Self {
            app,
            player,
            connection,
        }
    }

    fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    fn receive_bytes(&mut self, id: i32, bytes: &[u8]) {
        let event = ReceivedPacketEvent {
            entity: self.connection,
            id,
            data: Bytes::copy_from_slice(bytes),
            timestamp: Instant::now(),
        };
        self.world().trigger(event);
        self.world().flush();
    }

    fn receive_golden<P: Packet>(&mut self, name: &str) {
        self.receive_bytes(P::ID, &golden()[name]);
    }

    fn receive<P: Packet + Encode>(&mut self, packet: &P) {
        let mut bytes = Vec::new();
        packet.encode(&mut bytes).unwrap();
        self.receive_bytes(P::ID, &bytes);
    }

    fn raw(&self, path: &str, count: i32) -> RawStack {
        let id = registries()
            .id(
                "item",
                &mcrs_minecraft_core::ResourceLocation::minecraft(path).unwrap(),
            )
            .unwrap();
        let slot = ProtoStack::new(
            mcrs_minecraft_registry::Id::from_static(id),
            count,
            ComponentPatch::EMPTY,
        );
        RawStack::from_stack(&slot, registries()).unwrap()
    }

    fn cell(&mut self, holder: Entity, cell: u16) -> Option<Entity> {
        self.world().get::<SlotTable>(holder).unwrap().get(cell)
    }

    fn stack(&mut self, holder: Entity, cell: u16) -> (String, u8) {
        let entity = self
            .cell(holder, cell)
            .unwrap_or_else(|| panic!("cell {cell} is empty"));
        let stack = *self.world().get::<ItemStack>(entity).unwrap();
        let name = items()
            .get(stack.item)
            .unwrap()
            .identifier
            .path()
            .to_owned();
        (name, stack.count)
    }

    fn stacks(&mut self) -> usize {
        self.world()
            .query::<&ItemStack>()
            .iter(self.app.world())
            .count()
    }

    fn revision(&mut self, entity: Entity) -> u32 {
        self.world().get::<StackRevision>(entity).unwrap().0
    }

    fn seqno(&mut self, holder: Entity) -> u32 {
        self.world().get::<ContainerSeqno>(holder).unwrap().0
    }

    fn screen(&mut self) -> Screen {
        *self.world().resource::<Screen>()
    }

    fn open_chest(&mut self) -> Entity {
        self.receive_golden::<ClientboundOpenScreen>("open_screen");
        let Screen::Container(menu) = self.screen() else {
            panic!("open_screen opened no menu");
        };
        menu
    }

    fn open_menu(&mut self, menu_type: &'static str, container_id: i32) -> Entity {
        let id = registries()
            .id(
                "menu",
                &mcrs_minecraft_core::ResourceLocation::minecraft(menu_type).unwrap(),
            )
            .unwrap();
        self.receive(&ClientboundOpenScreen {
            container_id: VarInt(container_id),
            menu_type: RegistryId(id),
            title: Text::text(menu_type),
        });
        let Screen::Container(menu) = self.screen() else {
            panic!("open_screen opened no {menu_type}");
        };
        menu
    }
}

fn empty_player_content(count: usize) -> Client {
    let mut client = Client::new();
    client.receive_golden::<ClientboundContainerSetContent>("container_set_content");
    assert_eq!(client.stacks(), count);
    client
}

#[test]
fn clientbound_packets_drive_the_player_and_menu_tables() {
    set_content_fills_the_player_and_the_cursor();
    set_slot_on_an_occupied_cell_keeps_the_entity();
    an_empty_slot_despawns_the_stack();
    a_resync_replaces_the_whole_table();
    set_player_inventory_uses_the_inventory_index_space();
    set_cursor_item_lands_in_the_carried_cell();
    set_held_slot_accepts_only_the_hotbar();
    a_chest_lays_out_over_the_menu_and_the_player();
    closing_a_container_despawns_the_menu_and_keeps_the_player();
    opening_a_second_screen_replaces_the_first();
    a_menu_maps_its_slot_indices_onto_its_own_cells_and_the_player();
}

fn set_content_fills_the_player_and_the_cursor() {
    let mut client = empty_player_content(4);
    let player = client.player;
    assert_eq!(client.stack(player, 9), ("apple".into(), 3));
    assert_eq!(client.stack(player, 36), ("diamond_sword".into(), 1));
    assert_eq!(client.stack(player, 45), ("stone".into(), 16));
    assert_eq!(client.stack(player, slots::CARRIED), ("stone".into(), 64));
    assert_eq!(client.seqno(player), 5);

    let sword = client.cell(player, 36).unwrap();
    let apple = client.cell(player, 9).unwrap();
    let world = client.world();
    assert_eq!(
        world.get::<Held>(sword),
        Some(&Held {
            holder: player,
            index: 36
        })
    );
    assert_eq!(world.get::<Damage>(sword), Some(&Damage(Bounded(7))));
    assert_eq!(
        world.get::<CustomName>(sword),
        Some(&CustomName(Text::text("named")))
    );
    assert_eq!(world.get::<Unbreakable>(sword), Some(&Unbreakable));
    assert_eq!(world.get::<Damage>(apple), None);
}

fn set_slot_on_an_occupied_cell_keeps_the_entity() {
    let mut client = empty_player_content(4);
    let player = client.player;
    let apple = client.cell(player, 9).unwrap();
    let before = client.revision(apple);
    let raw = client.raw("apple", 7);
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(0),
        state_seqno: VarInt(6),
        slot: 9,
        item: raw,
    });
    assert_eq!(client.cell(player, 9), Some(apple));
    assert_eq!(client.stack(player, 9), ("apple".into(), 7));
    assert!(client.revision(apple) > before);
    assert_eq!(client.seqno(player), 6);
    assert_eq!(client.stacks(), 4);

    let raw = client.raw("diamond", 2);
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(0),
        state_seqno: VarInt(7),
        slot: 9,
        item: raw,
    });
    assert_ne!(client.cell(player, 9), Some(apple));
    assert_eq!(client.stack(player, 9), ("diamond".into(), 2));
    assert!(client.world().get_entity(apple).is_err());
    assert_eq!(client.stacks(), 4);
}

fn an_empty_slot_despawns_the_stack() {
    let mut client = empty_player_content(4);
    let player = client.player;
    let sword = client.cell(player, 36).unwrap();
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(0),
        state_seqno: VarInt(6),
        slot: 36,
        item: RawStack::EMPTY,
    });
    assert!(client.world().get_entity(sword).is_err());
    assert_eq!(client.cell(player, 36), None);
    assert_eq!(client.stacks(), 3);

    client.receive_golden::<ClientboundContainerSetSlot>("container_set_slot_empty");
    assert_eq!(client.stacks(), 3);
    assert_eq!(client.seqno(player), 6);
}

fn a_resync_replaces_the_whole_table() {
    let mut client = empty_player_content(4);
    let player = client.player;
    let raw = client.raw("apple", 1);
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(0),
        state_seqno: VarInt(6),
        slot: 10,
        item: raw,
    });
    let extra = client.cell(player, 10).unwrap();
    let apple = client.cell(player, 9).unwrap();
    let before = client.revision(apple);
    assert_eq!(client.stacks(), 5);

    client.receive_golden::<ClientboundContainerSetContent>("container_set_content");
    assert!(client.world().get_entity(extra).is_err());
    assert_eq!(client.cell(player, 10), None);
    assert_eq!(client.cell(player, 9), Some(apple));
    assert!(client.revision(apple) > before);
    assert_eq!(client.stacks(), 4);
    assert_eq!(client.seqno(player), 5);
}

fn set_player_inventory_uses_the_inventory_index_space() {
    assert_eq!(inventory_index_to_cell(0), Some(36));
    assert_eq!(inventory_index_to_cell(9), Some(9));
    assert_eq!(inventory_index_to_cell(36), Some(slots::ARMOR_FEET));
    assert_eq!(inventory_index_to_cell(39), Some(slots::ARMOR_HEAD));
    assert_eq!(inventory_index_to_cell(40), Some(slots::OFFHAND));
    assert_eq!(inventory_index_to_cell(41), None);
    assert_eq!(inventory_index_to_cell(-1), None);

    let mut client = Client::new();
    let player = client.player;
    client.receive_golden::<ClientboundSetPlayerInventory>("set_player_inventory");
    assert_eq!(client.stack(player, slots::ARMOR_FEET), ("apple".into(), 3));
    assert_eq!(client.stacks(), 1);
    client.receive(&ClientboundSetPlayerInventory {
        slot: VarInt(41),
        contents: client.raw("stone", 1),
    });
    assert_eq!(client.stacks(), 1);
}

fn set_cursor_item_lands_in_the_carried_cell() {
    let mut client = Client::new();
    let player = client.player;
    client.receive_golden::<ClientboundSetCursorItem>("set_cursor_item");
    assert_eq!(client.stack(player, slots::CARRIED), ("stone".into(), 64));
    client.receive(&ClientboundSetCursorItem {
        contents: RawStack::EMPTY,
    });
    assert_eq!(client.cell(player, slots::CARRIED), None);
    assert_eq!(client.stacks(), 0);
}

fn set_held_slot_accepts_only_the_hotbar() {
    let mut client = Client::new();
    let player = client.player;
    client.receive_golden::<ClientboundSetHeldSlot>("set_held_slot");
    assert_eq!(
        client.world().get::<SelectedHotbarSlot>(player),
        Some(&SelectedHotbarSlot(4))
    );
    client.receive(&ClientboundSetHeldSlot { slot: VarInt(9) });
    assert_eq!(
        client.world().get::<SelectedHotbarSlot>(player),
        Some(&SelectedHotbarSlot(4))
    );
    client.receive(&ClientboundSetHeldSlot { slot: VarInt(-1) });
    assert_eq!(
        client.world().get::<SelectedHotbarSlot>(player),
        Some(&SelectedHotbarSlot(4))
    );
}

fn a_chest_lays_out_over_the_menu_and_the_player() {
    let mut client = empty_player_content(4);
    let player = client.player;
    let menu = client.open_chest();
    {
        let world = client.world();
        let open = world.get::<OpenMenu>(menu).unwrap();
        assert_eq!(open.container_id, 1);
        assert_eq!(world.get::<SlotTable>(menu).unwrap().len(), 27);
        let layout = &world.get::<MenuLayout>(menu).unwrap().0;
        assert_eq!(layout.len(), 63);
        assert_eq!(layout[0], Slot::new(menu, 0));
        assert_eq!(layout[26], Slot::new(menu, 26));
        assert_eq!(layout[27], Slot::new(player, slots::MAIN.start));
        assert_eq!(layout[62], Slot::new(player, slots::HOTBAR.end - 1));
    }

    client.receive_golden::<ClientboundContainerSetContent>("container_set_content_chest");
    assert_eq!(client.stack(menu, 0), ("diamond".into(), 5));
    assert_eq!(client.stack(menu, 26), ("emerald".into(), 1));
    assert_eq!(client.stack(player, 9), ("apple".into(), 2));
    assert_eq!(client.stack(player, 44), ("stone".into(), 1));
    assert_eq!(client.cell(player, 36), None, "menu index 54 arrived empty");
    assert_eq!(
        client.stack(player, slots::OFFHAND),
        ("stone".into(), 16),
        "outside the layout"
    );
    assert_eq!(client.cell(player, slots::CARRIED), None);
    assert_eq!(client.seqno(menu), 2);
    assert_eq!(client.seqno(player), 5);
    assert_eq!(client.stacks(), 5);

    client.receive_golden::<ClientboundContainerSetSlot>("container_set_slot");
    assert_eq!(client.stacks(), 5, "container 3 is not open");
    assert_eq!(client.seqno(menu), 2);

    let raw = client.raw("emerald", 4);
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(1),
        state_seqno: VarInt(3),
        slot: 30,
        item: raw,
    });
    assert_eq!(client.stack(player, 12), ("emerald".into(), 4));
    assert_eq!(client.seqno(menu), 3);
    assert_eq!(client.seqno(player), 5);
}

fn closing_a_container_despawns_the_menu_and_keeps_the_player() {
    let mut client = empty_player_content(4);
    let player = client.player;
    let menu = client.open_chest();
    client.receive_golden::<ClientboundContainerSetContent>("container_set_content_chest");
    let diamond = client.cell(menu, 0).unwrap();
    let offhand = client.cell(player, slots::OFFHAND).unwrap();
    assert_eq!(client.stacks(), 5);

    client.receive_golden::<ClientboundContainerClose>("container_close");
    assert_eq!(client.screen(), Screen::None);
    assert!(client.world().get_entity(menu).is_err());
    assert!(client.world().get_entity(diamond).is_err());
    assert!(client.world().get_entity(offhand).is_ok());
    assert_eq!(client.stacks(), 3);
    assert_eq!(client.stack(player, 9), ("apple".into(), 2));
    assert_eq!(client.stack(player, 44), ("stone".into(), 1));

    client.receive_golden::<ClientboundContainerSetContent>("container_set_content_chest");
    assert_eq!(
        client.stacks(),
        3,
        "a closed container's content is ignored"
    );
}

fn opening_a_second_screen_replaces_the_first() {
    let mut client = Client::new();
    let first = client.open_chest();
    client.receive_golden::<ClientboundContainerSetContent>("container_set_content_chest");
    let second = client.open_chest();
    assert_ne!(first, second);
    assert!(client.world().get_entity(first).is_err());
    assert_eq!(
        client.stacks(),
        2,
        "the player's stacks survive, the chest's do not"
    );
}

fn a_menu_maps_its_slot_indices_onto_its_own_cells_and_the_player() {
    enum Lands {
        Menu(u16),
        Player(u16),
    }
    for (menu_type, layout, sent, lands) in [
        ("anvil", 39, 35, Lands::Player(slots::HOTBAR.start + 5)),
        ("anvil", 39, 2, Lands::Menu(2)),
        ("crafter_3x3", 46, 45, Lands::Menu(9)),
        ("lectern", 1, 0, Lands::Menu(0)),
    ] {
        let mut client = Client::new();
        let player = client.player;
        let menu = client.open_menu(menu_type, 4);
        assert_eq!(
            client.world().get::<MenuLayout>(menu).unwrap().0.len(),
            layout,
            "{menu_type}"
        );
        let raw = client.raw("stone", 3);
        client.receive(&ClientboundContainerSetSlot {
            container_id: VarInt(4),
            state_seqno: VarInt(1),
            slot: sent,
            item: raw,
        });
        let (holder, cell) = match lands {
            Lands::Menu(cell) => (menu, cell),
            Lands::Player(cell) => (player, cell),
        };
        assert_eq!(
            client.stack(holder, cell),
            ("stone".into(), 3),
            "{menu_type} slot {sent}"
        );
    }
}

#[test]
fn a_registry_the_server_sent_is_numbered_by_the_server() {
    let enchantment = |path: &str| ResourceLocation::minecraft(path).unwrap();
    let local = RegistrySet::from_tables(
        registries().tables().cloned().chain([Arc::new(
            NameTable::new(
                rl!("minecraft:enchantment").to_arc().into(),
                ["sharpness", "protection"].map(|path| enchantment(path).into()),
            )
            .unwrap(),
        )]),
    )
    .unwrap();
    let entry = |path: &str| RegistryEntry {
        id: enchantment(path).to_string(),
        data: None,
    };
    let mut received = ReceivedRegistries::default();
    received.push(ReceivedRegistry {
        registry: "minecraft:enchantment".to_owned(),
        entries: vec![entry("protection"), entry("sharpness")],
    });
    let mut patch = ComponentPatch::EMPTY;
    patch.set(Enchantments(vec![(
        ResourceKey::from_location(enchantment("protection")),
        1,
    )]));
    let sword = registries()
        .id("item", &rl!("minecraft:diamond_sword").to_arc())
        .unwrap();
    let raw = RawStack::from_stack(
        &ProtoStack::new(mcrs_minecraft_registry::Id::from_static(sword), 1, patch),
        &received.over(registries()),
    )
    .unwrap();

    let mut client = Client::with(local, received);
    let player = client.player;
    client.receive(&ClientboundContainerSetSlot {
        container_id: VarInt(0),
        state_seqno: VarInt(1),
        slot: 36,
        item: raw,
    });
    let stack = client.cell(player, 36).expect("the stack decoded");
    assert_eq!(
        client.world().get::<Enchantments>(stack),
        Some(&Enchantments(vec![(
            ResourceKey::from_location(enchantment("protection")),
            1
        )]))
    );
}
