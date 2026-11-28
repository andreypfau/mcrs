use std::num::NonZeroU8;

use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::EquippableSlot;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::apply;
use crate::fixture::dropped;
use crate::fixture::first_menu_sync;
use crate::fixture::holders;
use crate::fixture::saved;
use crate::fixture::stacks;

const SESSION: PlayerSession = PlayerSession(3);
pub(crate) const HOTBAR_0: u16 = 36;
const HOTBAR_1: u16 = 37;

pub(crate) fn joined(world: &mut World) -> Entity {
    let inventory = vec![ItemStackWithSlot {
        slot: 0,
        stack: saved("stone", 5),
    }];
    let mut file = PersistedSnapshot::new();
    file.put(&["Inventory"], &inventory).expect("the inventory");
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &file);
    world.run_schedule(FixedUpdate);
    sent(world);
    player
}

pub(crate) fn claim(kind: Kind, count: i32) -> Option<HashedStack> {
    Some(HashedStack {
        kind,
        count,
        values: Vec::new(),
        removals: Vec::new(),
    })
}

fn claim_of(world: &World, stack: &ItemStack) -> Option<HashedStack> {
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    hash_stack(Some(stack), &names, &mut HashScratch::default()).expect("the stack hashes")
}

pub(crate) struct Packet {
    container_id: i32,
    state_id: i32,
    slot: i16,
    kind: ClickKind,
    button: u8,
    changed: Vec<(u16, Option<HashedStack>)>,
    carried: Option<HashedStack>,
}

impl Packet {
    pub(crate) fn pick_up(state_id: i32, slot: u16) -> Self {
        Self {
            container_id: 0,
            state_id,
            slot: i16::try_from(slot).expect("a menu index"),
            kind: ClickKind::PickUp,
            button: 0,
            changed: Vec::new(),
            carried: None,
        }
    }

    pub(crate) fn drag_start(state_id: i32) -> Self {
        Self {
            slot: -999,
            kind: ClickKind::Drag,
            ..Self::pick_up(state_id, 0)
        }
    }

    pub(crate) fn pick_up_all(state_id: i32, slot: u16) -> Self {
        Self {
            kind: ClickKind::PickUpAll,
            ..Self::pick_up(state_id, slot)
        }
    }

    pub(crate) fn claiming(
        mut self,
        changed: Vec<(u16, Option<HashedStack>)>,
        carried: Option<HashedStack>,
    ) -> Self {
        self.changed = changed;
        self.carried = carried;
        self
    }

    pub(crate) fn in_container(mut self, container_id: i32) -> Self {
        self.container_id = container_id;
        self
    }
}

pub(crate) fn click(world: &mut World, player: Entity, packet: Packet) {
    let packet = ServerboundContainerClick {
        container_id: VarInt(packet.container_id),
        state_seqno: VarInt(packet.state_id),
        click: Click {
            kind: packet.kind,
            button: packet.button,
            clicked: Clicked::from(packet.slot),
        },
        changed_slots: packet.changed.into(),
        carried_item: packet.carried,
    };
    let mut body = Vec::new();
    packet.encode(&mut body).expect("the packet encodes");
    world.trigger(Received::<ServerboundContainerClick>::new(
        player,
        body.into(),
    ));
    world.flush();
}

pub(crate) fn summary(stack: &ProtoStack<Wire>) -> String {
    if stack.is_empty() {
        "empty".to_owned()
    } else {
        format!("{}x{}", stack.id.number(), stack.count)
    }
}

pub(crate) fn sent(world: &mut World) -> Vec<String> {
    let packets: Vec<OutboundPlayerPacket> = world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    packets
        .iter()
        .filter_map(|packet| match &packet.data {
            PacketPayload::ContainerSetContent(content) => {
                Some(format!("content state {}", content.state_seqno.0))
            }
            PacketPayload::ContainerSetSlot(slot) => Some(format!(
                "slot {} {} state {}",
                slot.slot,
                summary(&slot.item),
                slot.state_seqno.0
            )),
            PacketPayload::SetCursorItem(cursor) => {
                Some(format!("cursor {}", summary(&cursor.contents)))
            }
            PacketPayload::OpenScreen(screen) => Some(format!("open {}", screen.container_id.0)),
            PacketPayload::ContainerClose(close) => Some(format!("close {}", close.container_id.0)),
            _ => None,
        })
        .collect()
}

fn state_id(world: &World, player: Entity) -> u16 {
    first_menu_sync(world, player).state_id().get()
}

#[test]
fn itx_jav_004_an_applied_click_with_a_matching_claim_sends_nothing() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);

    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    world.run_schedule(FixedUpdate);

    let cursor = holders(world, player).cursor;
    assert_eq!(stacks(world, player), [], "P[0] gave its stack");
    assert_eq!(stacks(world, cursor), [(0, STONE, 5)], "the cursor took it");
    assert_eq!(sent(world), Vec::<String>::new());
    assert_eq!(state_id(world, player), 1, "no slot packet went out");
}

#[test]
fn itx_jav_004_a_matching_claim_of_a_stack_with_a_patch_sends_nothing() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let named = ItemStack::new(STONE, Count::new(3).expect("a count above 0"))
        .set(CustomName(Text::text("Rock")));
    let named_claim = claim_of(world, &named);
    world.commands().spawn_stack(named).put_at(player, 9);
    world.flush();
    world.run_schedule(FixedUpdate);
    sent(world);
    let state = state_id(world, player);

    click(
        world,
        player,
        Packet::pick_up(i32::from(state), HOTBAR_0)
            .claiming(vec![(HOTBAR_0, None), (9, named_claim)], claim(STONE, 5)),
    );
    world.run_schedule(FixedUpdate);

    assert_eq!(sent(world), Vec::<String>::new());
}

#[test]
fn itx_jav_004_a_click_that_the_client_got_wrong_is_corrected() {
    struct Case {
        name: &'static str,
        game_mode: GameMode,
        packet: Packet,
        sent: Vec<&'static str>,
        cursor: Vec<(usize, Kind, u8)>,
    }
    let cases = [
        Case {
            name: "a stale state id",
            game_mode: GameMode::Survival,
            packet: Packet::pick_up(0, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
            sent: vec!["content state 2"],
            cursor: vec![(0, STONE, 5)],
        },
        Case {
            name: "a wrong claim",
            game_mode: GameMode::Survival,
            packet: Packet::pick_up(1, HOTBAR_0)
                .claiming(vec![(HOTBAR_0, claim(STONE, 5))], claim(STONE, 4)),
            sent: vec!["slot 36 empty state 2", "cursor 1x5"],
            cursor: vec![(0, STONE, 5)],
        },
        Case {
            name: "a spectator",
            game_mode: GameMode::Spectator,
            packet: Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
            sent: vec!["content state 2"],
            cursor: vec![],
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        world
            .entity_mut(player)
            .insert(PlayerGameMode(case.game_mode));

        click(world, player, case.packet);
        world.run_schedule(FixedUpdate);

        let cursor = holders(world, player).cursor;
        assert_eq!(sent(world), case.sent, "{}", case.name);
        assert_eq!(stacks(world, cursor), case.cursor, "{}", case.name);
    }
}

#[test]
fn itx_jav_005_a_click_that_does_not_apply_is_corrected() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let empty_pick_up = Packet::pick_up(1, HOTBAR_1).claiming(vec![(HOTBAR_0, None)], None);

    click(world, player, empty_pick_up);
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, player), [(0, STONE, 5)]);
    assert_eq!(sent(world), ["slot 36 1x5 state 2"]);
}

#[test]
fn ien_spn_001_a_throw_click_drops_an_item_entity() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let throw = Packet {
        kind: ClickKind::Throw,
        button: 1,
        ..Packet::pick_up(1, HOTBAR_0)
    }
    .claiming(vec![(HOTBAR_0, None)], None);

    click(world, player, throw);
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, player), []);
    assert_eq!(dropped(world), [(STONE, 5, 40)]);
    assert_eq!(sent(world), Vec::<String>::new());
}

#[test]
fn itx_jav_003_each_click_reads_the_result_of_the_click_before_it() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);

    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_1).claiming(vec![(HOTBAR_1, claim(STONE, 5))], None),
    );
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, player), [(1, STONE, 5)]);
    assert_eq!(sent(world), Vec::<String>::new());
}

#[test]
fn itx_mod_005_a_drag_after_a_click_applies_in_the_same_tick() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let drag = |slot: i16, button: u8, changed: Vec<(u16, Option<HashedStack>)>, carried: i32| {
        Packet {
            slot,
            kind: ClickKind::Drag,
            button,
            ..Packet::pick_up(1, HOTBAR_0)
        }
        .claiming(changed, claim(STONE, carried))
    };

    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    click(world, player, drag(-999, 0, vec![], 5));
    for slot in [37, 38, 39] {
        click(world, player, drag(slot, 1, vec![], 5));
    }
    let spread = [37, 38, 39].map(|slot| (slot, claim(STONE, 1)));
    click(world, player, drag(-999, 2, spread.to_vec(), 2));
    world.run_schedule(FixedUpdate);

    let cursor = holders(world, player).cursor;
    assert_eq!(
        stacks(world, player),
        [(1, STONE, 1), (2, STONE, 1), (3, STONE, 1)]
    );
    assert_eq!(stacks(world, cursor), [(0, STONE, 2)]);
    assert_eq!(sent(world), Vec::<String>::new());
}

#[test]
fn inv_syn_004_a_change_outside_a_click_reaches_the_client() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let stone = ItemStack::new(STONE, Count::new(3).expect("a count above 0"));

    world.commands().entity(player).give(stone);
    world.flush();
    world.run_schedule(FixedUpdate);

    assert_eq!(sent(world), ["slot 36 1x8 state 2"]);
    assert_eq!(state_id(world, player), 2);
}

#[test]
fn itx_lim_005_a_quick_move_passes_over_a_slot_that_refuses_each_place() {
    struct Case {
        name: &'static str,
        locked: bool,
        inventory: Vec<(usize, Kind, u8)>,
        sent: Vec<&'static str>,
    }
    let cases = [
        Case {
            name: "P[9] open",
            locked: false,
            inventory: vec![(9, STONE, 5)],
            sent: vec![],
        },
        Case {
            name: "P[9] locked",
            locked: true,
            inventory: vec![(10, STONE, 5)],
            sent: vec!["slot 9 empty state 2", "slot 10 1x5 state 3"],
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        if case.locked {
            let mut policy = world
                .get_mut::<SlotPolicy>(player)
                .expect("a player inventory has a policy");
            let refuses_place = SlotAccess {
                limit: 99,
                take: Gate::Permits,
                place: Gate::Refuses,
            };
            policy.set(9, refuses_place);
        }
        let quick_move = Packet {
            kind: ClickKind::QuickMove,
            ..Packet::pick_up(1, HOTBAR_0)
        }
        .claiming(vec![(HOTBAR_0, None), (9, claim(STONE, 5))], None);

        click(world, player, quick_move);
        world.run_schedule(FixedUpdate);

        assert_eq!(stacks(world, player), case.inventory, "{}", case.name);
        assert_eq!(sent(world), case.sent, "{}", case.name);
    }
}

#[test]
fn inv_slt_003_a_helmet_enters_the_head_slot_by_a_click_and_by_a_quick_move() {
    const HEAD_ENTRY: u16 = 5;
    const FROM: u16 = 9;
    fn place(state_id: i32) -> Vec<Packet> {
        vec![
            Packet::pick_up(state_id, FROM),
            Packet::pick_up(state_id, HEAD_ENTRY),
        ]
    }
    fn quick_move(state_id: i32) -> Vec<Packet> {
        vec![Packet {
            kind: ClickKind::QuickMove,
            ..Packet::pick_up(state_id, FROM)
        }]
    }
    let paths: [(&str, fn(i32) -> Vec<Packet>); 2] =
        [("pick up and place", place), ("quick move", quick_move)];
    for (name, packets) in paths {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        let helmet = ItemStack::new(STONE, Count::new(1).expect("a count above 0"))
            .set(Equippable::new(EquippableSlot::Head));
        world.commands().spawn_stack(helmet).put_at(player, FROM);
        world.flush();
        world.run_schedule(FixedUpdate);
        sent(world);
        let state = i32::from(state_id(world, player));

        for packet in packets(state) {
            click(world, player, packet);
        }
        world.run_schedule(FixedUpdate);

        assert_eq!(
            stacks(world, player),
            [(0, STONE, 5), (39, STONE, 1)],
            "{name}"
        );
    }
}

#[test]
fn inv_slt_004_a_locked_armour_stack_leaves_the_head_slot_only_in_creative_mode() {
    const HEAD_ENTRY: u16 = 5;
    const HEAD: u16 = 39;
    struct Case {
        name: &'static str,
        enchantment: &'static str,
        game_mode: Option<GameMode>,
        inventory: Vec<(usize, Kind, u8)>,
    }
    let locked = || vec![(0, STONE, 5), (39, STONE, 1)];
    let taken = || vec![(0, STONE, 5)];
    let cases = [
        Case {
            name: "a curse in survival mode",
            enchantment: "minecraft:binding_curse",
            game_mode: Some(GameMode::Survival),
            inventory: locked(),
        },
        Case {
            name: "a curse without a game mode",
            enchantment: "minecraft:binding_curse",
            game_mode: None,
            inventory: locked(),
        },
        Case {
            name: "a curse in creative mode",
            enchantment: "minecraft:binding_curse",
            game_mode: Some(GameMode::Creative),
            inventory: taken(),
        },
        Case {
            name: "an enchantment without the effect",
            enchantment: "minecraft:protection",
            game_mode: Some(GameMode::Survival),
            inventory: taken(),
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        match case.game_mode {
            Some(game_mode) => world.entity_mut(player).insert(PlayerGameMode(game_mode)),
            None => world.entity_mut(player).remove::<PlayerGameMode>(),
        };
        let id = world
            .resource::<CurrentRegistries>()
            .id_of(
                ResourceLocation::read("minecraft:enchantment").expect("a registry name"),
                ResourceLocation::read(case.enchantment).expect("an enchantment name"),
            )
            .expect("the data pack has the enchantment");
        let enchantment = EnchantmentLevel {
            enchantment: EnchantmentId(id),
            level: NonZeroU8::MIN,
        };
        let helmet = ItemStack::new(STONE, Count::new(1).expect("a count above 0"))
            .set(Equippable::new(EquippableSlot::Head))
            .set(Enchantments::new([enchantment]).expect("one enchantment"));
        world.commands().spawn_stack(helmet).put_at(player, HEAD);
        world.flush();
        world.run_schedule(FixedUpdate);
        sent(world);
        let state = i32::from(state_id(world, player));

        click(world, player, Packet::pick_up(state, HEAD_ENTRY));
        world.run_schedule(FixedUpdate);

        assert_eq!(stacks(world, player), case.inventory, "{}", case.name);
    }
}
