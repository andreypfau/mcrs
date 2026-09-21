use std::path::PathBuf;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_assets::{RegistrySnapshotErased, snapshot::RegistrySnapshot};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ColumnPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_item::{
    DroppedItem, ItemStack, Items, Patch, SlotTable, mutate, slots, stack_to_slot,
};
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::dimension::Dimension;
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::{
    ComponentPatch, ContainerInput, Damage, HashedStack, ItemStackValue, ItemStackWithSlot,
    RawDelimitedStack, RawStack, ProtoStack,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundContainerClick, ServerboundSetCreativeModeSlot,
};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Encode, Packet};
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_server::WorldSave;
use mcrs_minecraft_server::runner::pump_channels;
use mcrs_minecraft_server::world::bus::{
    InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PacketTarget, PlayerTransferSnapshot,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::entity::item::spawn_dropped;
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::item::click::drop_stack;
use mcrs_minecraft_server::world::item::menu::{CurrentMenu, Menu};
use mcrs_minecraft_server::world::session::SessionBundle;
use mcrs_minecraft_server::world::sub_app_builder::DimSubAppHandle;
use mcrs_minecraft_world::save::{PlayerDat, read_player_dat, write_player_dat};

use crate::host_app;
use crate::inventory_sync::corpus;

const SPAWN: DVec3 = DVec3::new(8.5, 64.0, 8.5);

struct Server {
    app: App,
    dim: Entity,
    host_anchor: Entity,
    uuid: Uuid,
    save: PathBuf,
}

impl Server {
    fn start() -> Self {
        let (_, items) = corpus();
        let save = std::env::temp_dir().join(format!("mcrs-inventory-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&save).unwrap();
        let mut app = host_app::make_host_app();
        app.init_resource::<PlayerSessionCounter>();
        app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
        app.add_message::<InboundPlayerSpawn>();
        app.insert_resource(items.clone());
        app.insert_resource(WorldSave(save.clone()));
        app.world_mut()
            .resource_mut::<RegistryAccess>()
            .register(Box::new(RegistrySnapshotErased::from_entries(
                "minecraft:item",
                items
                    .0
                    .iter()
                    .map(|entry| (entry.identifier.clone(), None))
                    .collect(),
                None,
            )));
        app.insert_resource(RegistrySnapshot::<Biome>::default());
        host_app::drive_to_playing(&mut app);
        host_app::materialise_sub_apps(&mut app, &[("test:overworld", true)]);
        let dim = app
            .world_mut()
            .query_filtered::<Entity, With<DimSubAppHandle>>()
            .single(app.world())
            .unwrap();
        let host_anchor = app.world_mut().spawn_empty().id();
        Self {
            app,
            dim,
            host_anchor,
            uuid: Uuid::new_v4(),
            save,
        }
    }

    fn world(&mut self) -> &mut World {
        self.app.sub_app_mut(DimAppLabel(self.dim)).world_mut()
    }

    fn items(&mut self) -> Items {
        self.world().resource::<Items>().clone()
    }

    fn tick(&mut self) -> Vec<OutboundPlayerPacket> {
        self.app.update();
        pump_channels(&mut self.app);
        let anchor = self.host_anchor;
        self.app
            .world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .filter(|packet| match &packet.target {
                PacketTarget::SinglePlayer(target) => *target == anchor,
                PacketTarget::PlayerSet(targets) => targets.contains(&anchor),
                _ => true,
            })
            .collect()
    }

    fn ticks(&mut self, n: usize) -> Vec<OutboundPlayerPacket> {
        (0..n).flat_map(|_| self.tick()).collect()
    }

    fn control(&self, message: ToDim) {
        self.app
            .world()
            .resource::<DimChannelsResource>()
            .get(self.dim)
            .unwrap()
            .control_sender
            .try_send(message)
            .unwrap();
    }

    fn join(&mut self) -> Vec<OutboundPlayerPacket> {
        let session = self
            .app
            .world_mut()
            .resource_mut::<PlayerSessionCounter>()
            .next();
        self.app
            .world_mut()
            .entity_mut(self.host_anchor)
            .insert(SessionBundle::placed(
                session,
                SessionPlacement::new(Place::Joining(self.dim), 0),
            ));
        self.control(ToDim::Spawn {
            host_anchor: self.host_anchor,
            session: PlayerSession(0),
            snapshot: PlayerTransferSnapshot {
                uuid: self.uuid,
                username: "inventory".into(),
                position: SPAWN,
                rotation: bevy_math::Vec2::ZERO,
                view_distance: 2,
            },
            dimensions: Vec::new(),
        });
        self.ticks(2)
    }

    fn leave(&mut self) -> Vec<OutboundPlayerPacket> {
        self.control(ToDim::Despawn {
            host_anchor: self.host_anchor,
            session: PlayerSession(0),
        });
        self.ticks(2)
    }

    fn player(&mut self) -> Entity {
        let anchor = self.host_anchor;
        self.world()
            .query_filtered::<(Entity, &HostAnchor), With<Player>>()
            .iter(self.world())
            .find(|(_, host)| host.0 == anchor)
            .map(|(player, _)| player)
            .expect("the player is in the dimension")
    }

    fn send<P: Packet + Encode>(&self, packet: &P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        self.app
            .world()
            .resource::<DimChannelsResource>()
            .get(self.dim)
            .unwrap()
            .serverbound_sender
            .try_send(ToDim::Serverbound {
                player: self.host_anchor,
                id: P::ID,
                data: Bytes::from(data),
                timestamp: std::time::Instant::now(),
            })
            .unwrap();
    }

    fn state_id(&mut self) -> i32 {
        let player = self.player();
        let menu = self.world().get::<CurrentMenu>(player).unwrap().0;
        i32::from(self.world().get::<Menu>(menu).unwrap().state_id)
    }

    fn click(&self, state_id: i32, changed: Vec<(u16, Option<HashedStack>)>) {
        self.send(&ServerboundContainerClick {
            container_id: mcrs_minecraft_protocol::VarInt(0),
            state_seqno: mcrs_minecraft_protocol::VarInt(state_id),
            slot_index: -1,
            button: 0,
            container_input: ContainerInput::Pickup,
            changed_slots: mcrs_minecraft_protocol::Bounded(changed),
            carried_item: None,
        });
    }

    fn spawn(&mut self, item: &str, count: u8) -> Entity {
        let items = self.items();
        mutate::spawn_stack(self.world(), &value(item, count), &items).unwrap()
    }

    fn give(&mut self, item: &str, count: u8, cell: u16) -> Entity {
        let player = self.player();
        let stack = self.spawn(item, count);
        mutate::move_stack(self.world(), stack, player, cell).unwrap();
        stack
    }

    fn cell(&mut self, cell: u16) -> Option<(Entity, u8)> {
        let player = self.player();
        let stack = self.world().get::<SlotTable>(player).unwrap().get(cell)?;
        Some((stack, self.world().get::<ItemStack>(stack).unwrap().count()))
    }

    /// The client holds the player's own column, so entities standing in it pair with it.
    fn observe_own_column(&mut self) {
        let player = self.player();
        let column_pos = ColumnPos::from(SPAWN);
        let world = self.world();
        let dim = world
            .query_filtered::<Entity, With<Dimension>>()
            .single(world)
            .unwrap();
        let existing = world
            .get::<ColumnIndex>(dim)
            .unwrap()
            .0
            .get(&column_pos)
            .map(|slot| slot.entity);
        let column = match existing {
            Some(column) => column,
            None => {
                let column = world.spawn((Column, PlayerObservers::default())).id();
                world.get_mut::<ColumnIndex>(dim).unwrap().0.insert(
                    column_pos,
                    ColumnSlot {
                        entity: column,
                        section_count: 0,
                    },
                );
                column
            }
        };
        world
            .get_mut::<PlayerObservers>(column)
            .unwrap()
            .0
            .push(player);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.save);
    }
}

fn value(item: &str, count: u8) -> ItemStackValue {
    ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft(item)),
        count: Bounded(i32::from(count)),
        components: ComponentPatch::EMPTY,
    }
}

fn inventory_packets(packets: &[OutboundPlayerPacket]) -> Vec<&PacketPayload> {
    packets
        .iter()
        .map(|packet| &packet.data)
        .filter(|data| {
            matches!(
                data,
                PacketPayload::SetHeldSlot(_)
                    | PacketPayload::ContainerSetContent { .. }
                    | PacketPayload::ContainerSetSlot { .. }
                    | PacketPayload::SetCursorItem(_)
            )
        })
        .collect()
}

fn set_slots<'a>(
    packets: &'a [OutboundPlayerPacket],
) -> impl Iterator<Item = (i16, &'a RawStack)> + 'a {
    packets.iter().filter_map(|packet| match &packet.data {
        PacketPayload::ContainerSetSlot {
            container_id: 0,
            slot,
            item,
            ..
        } => Some((*slot, item)),
        _ => None,
    })
}

#[test]
fn join_sends_held_slot_then_the_full_inventory_after_the_login_packets() {
    let mut server = Server::start();
    let packets = server.join();
    let login = packets
        .iter()
        .position(|packet| matches!(packet.data, PacketPayload::PlayerLogin { .. }))
        .expect("login is sent");
    let held = packets
        .iter()
        .position(|packet| matches!(packet.data, PacketPayload::SetHeldSlot(0)))
        .expect("the held slot is sent");
    assert!(login < held, "{packets:?}");
    let inventory = inventory_packets(&packets);
    assert_eq!(inventory.len(), 2, "{inventory:?}");
    assert!(matches!(inventory[0], PacketPayload::SetHeldSlot(0)));
    match inventory[1] {
        PacketPayload::ContainerSetContent {
            container_id: 0,
            state_id: 1,
            slots,
            carried,
        } => {
            assert_eq!(slots.len(), slots::MENU_COUNT);
            assert!(slots.iter().all(|slot| *slot == RawStack::EMPTY));
            assert_eq!(*carried, RawStack::EMPTY);
        }
        other => panic!("{other:?}"),
    }
    assert!(inventory_packets(&server.ticks(3)).is_empty());
}

#[test]
fn a_creative_slot_is_answered_with_one_set_slot() {
    let mut server = Server::start();
    server.join();
    let items = server.items();
    let registry = server.world().resource::<RegistryAccess>().clone();
    let slot = ProtoStack::from_value(&value("diamond_pickaxe", 1), &registry as &dyn RegistryLookup).unwrap();
    let cell = slots::held(0);
    server.send(&ServerboundSetCreativeModeSlot {
        slot: cell as i16,
        item: RawDelimitedStack::from_stack(&slot, &registry).unwrap(),
    });
    let packets = server.ticks(3);
    let inventory = inventory_packets(&packets);
    assert_eq!(inventory.len(), 1, "{inventory:?}");
    let PacketPayload::ContainerSetSlot {
        container_id: 0,
        state_id: 2,
        slot: sent_slot,
        item,
    } = inventory[0]
    else {
        panic!("{inventory:?}");
    };
    assert_eq!(*sent_slot, cell as i16);
    let (stack, count) = server.cell(cell).unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        item.resolve(&registry).unwrap(),
        stack_to_slot(server.world(), stack, &items)
    );
}

#[test]
fn a_click_whose_claim_disagrees_gets_that_cell_resent() {
    let mut server = Server::start();
    server.join();
    let items = server.items();
    let stone = server.give("stone", 7, slots::HOTBAR.start);
    server.ticks(2);
    let claimed = HashedStack::create(&stack_to_slot(server.world(), stone, &items)).unwrap();
    let untouched = slots::MAIN.start + 3;
    let state_id = server.state_id();
    server.click(state_id, vec![(untouched, claimed)]);
    let packets = server.ticks(3);
    let resent: Vec<_> = set_slots(&packets).collect();
    assert_eq!(resent.len(), 1, "{packets:?}");
    assert_eq!(resent[0], (untouched as i16, &RawStack::EMPTY));
}

#[test]
fn a_stale_state_id_resends_the_whole_menu() {
    let mut server = Server::start();
    server.join();
    server.give("stone", 7, slots::HOTBAR.start);
    server.ticks(2);
    let state_id = server.state_id();
    server.click(state_id - 1, Vec::new());
    let packets = server.ticks(3);
    let inventory = inventory_packets(&packets);
    assert_eq!(inventory.len(), 1, "{inventory:?}");
    let PacketPayload::ContainerSetContent {
        container_id: 0,
        state_id: sent,
        slots,
        ..
    } = inventory[0]
    else {
        panic!("{inventory:?}");
    };
    assert_eq!(i32::from(*sent), state_id + 1);
    assert_ne!(slots[slots::HOTBAR.start as usize], RawStack::EMPTY);
}

#[test]
fn damaging_a_pickaxe_inside_a_shulker_resends_the_shulker_cell() {
    let mut server = Server::start();
    server.join();
    let items = server.items();
    let cell = slots::HOTBAR.start + 2;
    let shulker = server.give("shulker_box", 1, cell);
    let pickaxe = server.spawn("diamond_pickaxe", 1);
    mutate::move_stack(server.world(), pickaxe, shulker, 4).unwrap();
    let before: Vec<_> = set_slots(&server.ticks(2)).map(|(slot, _)| slot).collect();
    assert_eq!(before, vec![cell as i16]);

    mutate::set(server.world(), pickaxe, Damage(Bounded(3)), &items);
    let packets = server.ticks(2);
    let resent: Vec<_> = set_slots(&packets).collect();
    assert_eq!(resent.len(), 1, "{packets:?}");
    assert_eq!(resent[0].0, cell as i16);
    let registry = server.world().resource::<RegistryAccess>().clone();
    let sent = resent[0].1.resolve(&registry).unwrap();
    assert_eq!(sent, stack_to_slot(server.world(), shulker, &items));
    assert_eq!(
        server.world().get::<Patch<Damage>>(pickaxe),
        Some(&Patch(Some(Damage(Bounded(3)))))
    );
}

#[test]
fn a_drop_adds_an_item_entity_and_its_stack_metadata() {
    let mut server = Server::start();
    server.join();
    server.observe_own_column();
    let player = server.player();
    let stone = server.give("stone", 7, slots::HOTBAR.start);
    server.ticks(2);

    drop_stack(server.world(), player, stone);
    assert_eq!(
        server.world().get::<DroppedItem>(stone).unwrap().pickup_delay,
        40
    );
    let packets = server.ticks(2);
    let wire_id = stone.index_u32() as i32;
    let added = packets.iter().position(|packet| {
        matches!(
            packet.data,
            PacketPayload::PlayerEnteredView { entity_id, kind: 72, .. } if entity_id == wire_id
        )
    });
    let data = packets.iter().position(|packet| {
        matches!(
            &packet.data,
            PacketPayload::SetEntityData { entity_id, metadata }
                if *entity_id == wire_id && metadata.0.iter().any(|entry| entry.index == 8)
        )
    });
    assert!(added.is_some() && data.is_some(), "{packets:?}");
    assert!(added < data, "{packets:?}");
    assert_eq!(server.cell(slots::HOTBAR.start), None);
}

#[test]
fn a_pickup_announces_the_full_take_then_fills_the_held_stack_and_a_free_cell() {
    let mut server = Server::start();
    server.join();
    let player = server.player();
    let held = server.give("stone", 60, slots::held(0));
    server.ticks(2);
    let dim = server
        .world()
        .get::<mcrs_minecraft_level::world::dimension::InDimension>(player)
        .unwrap()
        .0;
    let item = server.spawn("stone", 7);
    spawn_dropped(server.world(), item, dim, SPAWN, DVec3::ZERO, 0, None);

    let packets = server.ticks(2);
    let take = packets
        .iter()
        .position(|packet| {
            matches!(
                packet.data,
                PacketPayload::TakeItemEntity { item_id, player_id, amount: 7 }
                    if item_id == item.index_u32() as i32 && player_id == player.index_u32() as i32
            )
        })
        .unwrap_or_else(|| panic!("{packets:?}"));
    let resent: Vec<i16> = set_slots(&packets).map(|(slot, _)| slot).collect();
    assert_eq!(
        resent,
        vec![slots::held(0) as i16, slots::held(1) as i16],
        "{packets:?}"
    );
    assert!(take < packets.len() - 2, "{packets:?}");
    assert_eq!(server.cell(slots::held(0)), Some((held, 64)));
    assert_eq!(server.cell(slots::held(1)).map(|cell| cell.1), Some(3));
    assert!(server.world().get_entity(item).is_err());
}

#[test]
fn a_relog_round_trips_the_player_file_with_keys_it_does_not_model() {
    let mut server = Server::start();
    let mut planted = NbtCompound::new();
    planted.put_int("foodLevel", 17);
    let mut dat = PlayerDat {
        rest: planted.clone(),
        ..PlayerDat::default()
    };
    dat.inventory.push(ItemStackWithSlot {
        slot: 3,
        stack: value("diamond_pickaxe", 1),
    });
    dat.selected_item_slot = 3;
    write_player_dat(&server.save, server.uuid, &dat).unwrap();

    let packets = server.join();
    let held = packets
        .iter()
        .find_map(|packet| match packet.data {
            PacketPayload::SetHeldSlot(slot) => Some(slot),
            _ => None,
        })
        .unwrap();
    assert_eq!(held, 3);
    let content = packets
        .iter()
        .find_map(|packet| match &packet.data {
            PacketPayload::ContainerSetContent { slots, .. } => Some(slots),
            _ => None,
        })
        .unwrap();
    assert_ne!(content[slots::held(3) as usize], RawStack::EMPTY);
    assert_eq!(
        inventory_packets(&packets).len(),
        2,
        "a loaded inventory is sent once, in the full content"
    );
    assert_eq!(server.cell(slots::held(3)).map(|cell| cell.1), Some(1));

    server.give("stone", 5, slots::MAIN.start);
    server.ticks(1);
    server.leave();
    assert!(
        server
            .world()
            .query_filtered::<Entity, With<Player>>()
            .iter(server.world())
            .next()
            .is_none()
    );
    let saved = read_player_dat(&server.save, server.uuid).unwrap().unwrap();
    assert_eq!(saved.selected_item_slot, 3);
    assert_eq!(saved.rest, planted);
    let mut inventory = saved.inventory.clone();
    inventory.sort_by_key(|entry| entry.slot);
    assert_eq!(
        inventory,
        vec![
            ItemStackWithSlot {
                slot: 3,
                stack: value("diamond_pickaxe", 1)
            },
            ItemStackWithSlot {
                slot: 9,
                stack: value("stone", 5)
            },
        ]
    );

    let packets = server.join();
    let content = packets
        .iter()
        .find_map(|packet| match &packet.data {
            PacketPayload::ContainerSetContent { slots, .. } => Some(slots),
            _ => None,
        })
        .unwrap();
    assert_ne!(content[slots::held(3) as usize], RawStack::EMPTY);
    assert_ne!(content[slots::MAIN.start as usize], RawStack::EMPTY);
}
