use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::write_hashed;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::item::ContainerInput;
use mcrs_minecraft_protocol_game::item::HashedStack;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_vanilla_inventory::InventoryMenuSync;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::apply;
use crate::fixture::dropped;
use crate::fixture::holders;
use crate::fixture::saved;
use crate::fixture::stacks;

const SESSION: PlayerSession = PlayerSession(3);
/// The menu index of `P[0]`, which holds five stone after the join.
const HOTBAR_0: u16 = 36;
const HOTBAR_1: u16 = 37;

/// A player with five stone in `P[0]`, after the send of its full state under the state id 1.
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

fn claim(kind: Kind, count: i32) -> Option<HashedStack> {
    let hashed = mcrs_minecraft_item_stack::HashedStack {
        kind,
        count,
        values: Vec::new(),
        removals: Vec::new(),
    };
    let mut bytes = Vec::new();
    write_hashed(Some(&hashed), &mut bytes).expect("the hashed stack encodes");
    Option::<HashedStack>::decode(&mut bytes.as_slice()).expect("the hashed stack decodes")
}

struct Packet {
    state_id: i32,
    slot: i16,
    input: ContainerInput,
    button: u8,
    changed: Vec<(u16, Option<HashedStack>)>,
    carried: Option<HashedStack>,
}

impl Packet {
    /// A pick up with button 0 on `slot`.
    fn pick_up(state_id: i32, slot: u16) -> Self {
        Self {
            state_id,
            slot: i16::try_from(slot).expect("a menu index"),
            input: ContainerInput::Pickup,
            button: 0,
            changed: Vec::new(),
            carried: None,
        }
    }

    fn claiming(
        mut self,
        changed: Vec<(u16, Option<HashedStack>)>,
        carried: Option<HashedStack>,
    ) -> Self {
        self.changed = changed;
        self.carried = carried;
        self
    }
}

fn click(world: &mut World, player: Entity, packet: Packet) {
    let packet = ServerboundContainerClick {
        container_id: VarInt(0),
        state_seqno: VarInt(packet.state_id),
        slot_index: packet.slot,
        button: packet.button,
        container_input: packet.input,
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

fn summary(stack: &ProtoStack<Wire>) -> String {
    if stack.is_empty() {
        "empty".to_owned()
    } else {
        format!("{}x{}", stack.id.number(), stack.count)
    }
}

/// Each inventory packet that the tick sent, as text.
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
            _ => None,
        })
        .collect()
}

fn state_id(world: &World, player: Entity) -> u16 {
    world
        .get::<InventoryMenuSync>(player)
        .expect("a joined player has a client view")
        .state_id()
        .get()
}

/// ITX-JAV-002, ITX-JAV-004, INV-SYN-003: a pick up moves the stack to the cursor through the
/// engine, and the server sends nothing when the claim of the client matches.
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

/// ITX-JAV-001 check 2, ITX-JAV-004, INV-SYN-001, INV-SYN-005: a stale state id or a spectator
/// gets the full content; a claim that does not match gets the slot and the cursor that differ.
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

/// ITX-JAV-005, ITX-ATM-007: a click whose request does not apply changes no slot, and the
/// client gets the stack that the server holds.
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

/// INV-MNU-006, IEN-SPN-001: a throw click drops the stack through the engine as an item entity
/// with the pickup delay of a throw.
#[test]
fn ien_spn_001_a_throw_click_drops_an_item_entity() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let throw = Packet {
        input: ContainerInput::Throw,
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

/// ITX-JAV-003, ITX-MOD-005: the second click of one tick reads the slots that the request of the
/// first left, so a pick up and a place move the stack.
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
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, player), [(1, STONE, 5)]);
    assert_eq!(sent(world), Vec::<String>::new());
}

/// INV-SYN-004: a give, which is no click, reaches the client by the comparison.
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
