use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use bevy_time::TimeUpdateStrategy;
use bytes::Bytes;
use mcrs_command::Limits;
use mcrs_minecraft_command_engine::{CommandLimits, CommandPlugin};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::column_version::ColumnVersion;
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClockPlugin;
use mcrs_minecraft_engine::coordinator::world_clock_host_plugin::WorldClockHostPlugin;
use mcrs_minecraft_engine::player_state::{Effect, PlayerFileState};
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine_item::{ItemCommandsExt, StackCommandsExt};
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::world::storage::column::ColumnPos;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_environment::{AdvanceTime, WorldClockPlugin};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{
    AttackAnimation, Count, InteractAnimation, ItemStack, Kind, PiercingWeapon, SwingAnimation,
    SwingKind,
};
use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSwingAnimation;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChatCommand, ServerboundChunkBatchReceived,
    ServerboundClientInformation, ServerboundPlayerAction, ServerboundPlayerLoaded,
    ServerboundPunch, ServerboundSetCarriedItem, ServerboundUseItemOn,
};
use mcrs_minecraft_protocol_game::{Direction, Hand, SwingAnimationKind};
use mcrs_minecraft_server::operator_broadcast::OperatorBroadcastPlugin;
use mcrs_minecraft_server::{
    VanillaBlockPlacePlugin, VanillaBlocksPlugin, VanillaEntityPhysicsPlugin,
    VanillaItemEntityPlugin, VanillaItemPlugin, VanillaSwingPlugin,
};
use mcrs_voxel::BlockPos;

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer, label_of, replicated, sent_to};

const ALICE: Uuid = Uuid::from_u128(101);
const BOB: Uuid = Uuid::from_u128(202);
const CHARLIE: Uuid = Uuid::from_u128(303);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObservedSwing {
    entity_id: i32,
    hand: Hand,
    animation: SwingAnimationKind,
    duration: i32,
}

fn start_swing_server(dimensions: &[(&str, &str)]) -> PlayServer {
    let mut app = host_app::make_host_app();
    install_item_table(app.world_mut(), test_item_table()).unwrap();
    app.insert_resource(CommandLimits(Limits::default()));
    app.add_plugins(WorldClockPlugin);
    app.insert_resource(AdvanceTime(false));
    app.add_plugins(WorldClockHostPlugin);
    app.add_plugins(BirthClockPlugin);
    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_dimension_plugin(OperatorBroadcastPlugin);
    app.add_plugins((
        VanillaEntityPhysicsPlugin,
        VanillaBlocksPlugin::default(),
        VanillaItemEntityPlugin,
        VanillaItemPlugin,
        VanillaBlockPlacePlugin,
        VanillaSwingPlugin,
        VanillaCommandsPlugin,
    ));
    let mut server = PlayServer::start_in(app, dimensions, &[(ALICE, 2)], 0);
    load_stone_column(&mut server);
    server
}

fn step_and_collect(
    server: &mut PlayServer,
    anchors: &[Entity],
    ticks: usize,
) -> Vec<Vec<PacketPayload>> {
    let raw = server.ticks_all(ticks);
    anchors
        .iter()
        .map(|&anchor| {
            let session = server.session_of(anchor).unwrap();
            sent_to(session, raw.clone())
        })
        .collect()
}

fn extract_swings(packets: &[PacketPayload]) -> Vec<ObservedSwing> {
    let mut out = Vec::new();
    for packet in packets {
        if matches!(packet, PacketPayload::Replication(_)) {
            for (id, bytes) in replicated(std::slice::from_ref(packet)) {
                if id == ClientboundSwingAnimation::ID {
                    let mut slice: &[u8] = &bytes;
                    let swing = ClientboundSwingAnimation::decode(&mut slice).unwrap();
                    out.push(ObservedSwing {
                        entity_id: swing.entity_id.0,
                        hand: swing.hand,
                        animation: swing.animation,
                        duration: swing.duration.0,
                    });
                }
            }
        }
    }
    out
}

fn extract_order(packets: &[PacketPayload]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for packet in packets {
        match packet {
            PacketPayload::BlockUpdate(_) | PacketPayload::SectionBlocksUpdate(_) => {
                out.push("block_update");
            }
            PacketPayload::Replication(_) => {
                for (id, _) in replicated(std::slice::from_ref(packet)) {
                    if id == ClientboundSwingAnimation::ID {
                        out.push("swing");
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn action(action: PlayerAction) -> ServerboundPlayerAction {
    ServerboundPlayerAction {
        action,
        pos: BlockPos::new(0, 0, 0),
        direction: Direction::Up,
        sequence: VarInt(1),
    }
}

fn confirm_move(server: &mut PlayServer, anchor: Entity, dim: Entity, to: DVec3) {
    let player = server.player_in(dim, anchor).unwrap();
    server
        .world_in(dim)
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = to;
    let _ = server.ticks_all(2);
    if let Some(id) = server
        .world_in(dim)
        .get::<TeleportState>(player)
        .and_then(|teleport| teleport.pending())
        .map(|pending| pending.id)
    {
        server.queue(
            anchor,
            &ServerboundAcceptTeleportation {
                teleport_id: VarInt(id),
                position: to.into(),
                look: Look::default(),
            },
        );
        let _ = server.ticks_all(2);
    }
}

struct JoinedPair {
    alice: Entity,
    bob: Entity,
    alice_entity: Entity,
    bob_entity: Entity,
}

fn join_pair(server: &mut PlayServer) -> JoinedPair {
    let (alice, _) = server.join(ALICE, "Alice");
    let (bob, _) = server.join(BOB, "Bob");
    let _ = server.ticks_all(5);
    JoinedPair {
        alice,
        bob,
        alice_entity: server.player(alice),
        bob_entity: server.player(bob),
    }
}

fn push_effect(server: &mut PlayServer, player: Entity, id: &str, amplifier: u8, duration: i32) {
    server
        .world()
        .get_mut::<PlayerFileState>(player)
        .unwrap()
        .active_effects
        .push(Effect {
            id: id.to_owned(),
            amplifier,
            duration,
            ambient: false,
            show_particles: false,
            show_icon: Some(false),
            hidden_effect: None,
        });
}

#[test]
fn swg_loaded_client_punch_fallback_and_main_arm_invariance() {
    let mut server = start_swing_server(&[("minecraft:overworld", "minecraft:overworld")]);
    let pair = join_pair(&mut server);

    server.queue(pair.alice, &ServerboundPunch);
    let packets = step_and_collect(&mut server, &[pair.alice, pair.bob], 1);
    assert!(
        extract_swings(&packets[0]).is_empty(),
        "actor never receives observer-only punch"
    );
    assert!(
        extract_swings(&packets[1]).is_empty(),
        "SWG-SRC-002: punch before ServerboundPlayerLoaded or 60-tick fallback must be ignored"
    );

    server.queue(pair.alice, &ServerboundPlayerLoaded);
    server.queue(
        pair.alice,
        &ServerboundClientInformation(ClientInformation {
            locale: "en_us",
            view_distance: 2,
            chat_mode: ChatMode::Enabled,
            chat_colors: true,
            displayed_skin_parts: DisplayedSkinParts::new(),
            main_arm: MainArm::Left,
            enable_text_filtering: false,
            allow_server_listings: true,
            particle_status: ParticleStatus::All,
        }),
    );
    let _ = server.ticks_all(1);

    server.queue(pair.alice, &ServerboundPunch);
    let packets = step_and_collect(&mut server, &[pair.alice, pair.bob], 1);
    assert!(
        extract_swings(&packets[0]).is_empty(),
        "SWG-SRC-001: punch is observer-only"
    );
    let bob_swings = extract_swings(&packets[1]);
    assert_eq!(bob_swings.len(), 1);
    assert_eq!(
        (
            bob_swings[0].hand,
            bob_swings[0].animation,
            bob_swings[0].duration
        ),
        (Hand::Main, SwingAnimationKind::Whack, 6),
        "SWG-PKT-001: left main_arm still wires as Main"
    );

    let _ = server.ticks_all(55);
    server.queue(pair.bob, &ServerboundPunch);
    let packets = step_and_collect(&mut server, &[pair.alice, pair.bob], 1);
    assert_eq!(
        extract_swings(&packets[0]).len(),
        1,
        "SWG-SRC-001: 60-tick fallback enables punch without ServerboundPlayerLoaded"
    );
}

fn equip_test_items(server: &mut PlayServer, player: Entity) {
    let stick = Kind(Item::Stick.id().number());
    let items = [
        (
            0,
            ItemStack::new(stick, Count::new(1).unwrap()).set(AttackAnimation(SwingAnimation {
                kind: SwingKind::None,
                duration: 10,
            })),
        ),
        (
            1,
            ItemStack::new(stick, Count::new(3).unwrap())
                .set(PiercingWeapon::new())
                .set(AttackAnimation(SwingAnimation {
                    kind: SwingKind::Stab,
                    duration: 12,
                })),
        ),
        (
            2,
            ItemStack::new(stick, Count::new(1).unwrap()).set(AttackAnimation(SwingAnimation {
                kind: SwingKind::Whack,
                duration: 0,
            })),
        ),
    ];
    let world = server.world();
    for (slot, stack) in items {
        world.commands().spawn_stack(stack).put_at(player, slot);
    }
    world.flush();
}

fn swings_for(server: &mut PlayServer, viewer: Entity) -> Vec<ObservedSwing> {
    extract_swings(&step_and_collect(server, &[viewer], 1)[0])
}

#[test]
fn swg_item_components_effects_and_cooldowns() {
    let mut server = start_swing_server(&[("minecraft:overworld", "minecraft:overworld")]);
    let pair = join_pair(&mut server);
    server.queue(pair.alice, &ServerboundPlayerLoaded);
    let _ = server.ticks_all(1);
    equip_test_items(&mut server, pair.alice_entity);

    server.queue(pair.alice, &ServerboundPunch);
    let swing = swings_for(&mut server, pair.bob)[0];
    assert_eq!(
        (swing.animation, swing.duration),
        (SwingAnimationKind::None, 10)
    );
    for _ in 0..5 {
        server.queue(pair.alice, &ServerboundPunch);
        assert!(
            swings_for(&mut server, pair.bob).is_empty(),
            "SWG-STA-005: cooldown must block ticks T+1..T+5"
        );
    }

    server.queue(pair.alice, &ServerboundSetCarriedItem { slot: 1 });
    server.queue(pair.alice, &action(PlayerAction::Stab));
    let swing = swings_for(&mut server, pair.bob)[0];
    assert_eq!(
        (swing.animation, swing.duration),
        (SwingAnimationKind::Stab, 12),
        "SWG-SRC-003: stab must work immediately on same tick as equip"
    );

    let _ = server.ticks_all(7);
    server.queue(pair.alice, &ServerboundPunch);
    server.queue(pair.alice, &action(PlayerAction::DropItem));
    assert_eq!(
        swings_for(&mut server, pair.bob).len(),
        1,
        "SWG-STA-004: at most one swing per actor per tick"
    );

    let _ = server.ticks_all(7);
    server.queue(pair.alice, &action(PlayerAction::DropItem));
    let bob_swings = swings_for(&mut server, pair.bob);
    assert_eq!(bob_swings.len(), 1);
    assert_eq!(
        (
            bob_swings[0].hand,
            bob_swings[0].animation,
            bob_swings[0].duration
        ),
        (Hand::Main, SwingAnimationKind::Whack, 6),
        "SWG-CMP-002: drop must use default main whack 6 regardless of item components"
    );

    server.queue(pair.alice, &ServerboundSetCarriedItem { slot: 2 });
    let _ = server.ticks_all(4);
    server.queue(pair.alice, &ServerboundPunch);
    assert_eq!(swings_for(&mut server, pair.bob)[0].duration, 0);
    server.queue(pair.alice, &ServerboundPunch);
    assert_eq!(
        swings_for(&mut server, pair.bob).len(),
        1,
        "SWG-STA-003: duration 0 must unlock immediately at T + 1"
    );

    server.queue(pair.alice, &ServerboundSetCarriedItem { slot: 3 });
    push_effect(
        &mut server,
        pair.alice_entity,
        "minecraft:mining_fatigue",
        1,
        10,
    );
    server.queue(pair.alice, &ServerboundPunch);
    assert_eq!(
        swings_for(&mut server, pair.bob)[0].duration,
        6,
        "SWG-CMP-003: wire duration must stay unscaled under mining fatigue"
    );
    for _ in 0..5 {
        server.queue(pair.alice, &ServerboundPunch);
        assert!(
            swings_for(&mut server, pair.bob).is_empty(),
            "SWG-STA-002: fatigue D=10 blocks up to T+5"
        );
    }
    server.queue(pair.alice, &ServerboundPunch);
    assert_eq!(
        swings_for(&mut server, pair.bob).len(),
        1,
        "SWG-STA-002: fatigue D=10 unlocks at T+6"
    );
}

#[test]
fn swg_spectator_range_late_viewer_and_dimension_change() {
    let mut server = start_swing_server(&[
        ("minecraft:overworld", "minecraft:overworld"),
        ("minecraft:the_nether", "minecraft:the_nether"),
    ]);
    let pair = join_pair(&mut server);
    server.queue(pair.alice, &ServerboundPlayerLoaded);
    server.queue(pair.bob, &ServerboundPlayerLoaded);
    let _ = server.ticks_all(1);

    server
        .world()
        .get_mut::<PlayerGameMode>(pair.bob_entity)
        .unwrap()
        .0 = GameMode::Spectator;
    let _ = server.ticks_all(1);
    server.queue(pair.bob, &ServerboundPunch);
    assert!(
        extract_swings(&step_and_collect(&mut server, &[pair.alice, pair.bob], 1)[0]).is_empty(),
        "SWG-SRC-002: spectator punch must emit nothing"
    );
    server.queue(pair.alice, &ServerboundPunch);
    assert_eq!(
        extract_swings(&step_and_collect(&mut server, &[pair.alice, pair.bob], 1)[1]).len(),
        1,
        "SWG-DST-001: spectator viewer still receives observer swings"
    );

    let (charlie, charlie_packets) = server.join(CHARLIE, "Charlie");
    assert!(
        extract_swings(&charlie_packets).is_empty(),
        "SWG-DST-002: late viewer joining must never replay old swings"
    );

    let dim = server.dim;
    confirm_move(&mut server, charlie, dim, DVec3::new(500.5, 64.0, 500.5));
    let _ = server.ticks_all(6);
    server.queue(pair.alice, &ServerboundPunch);
    let packets = step_and_collect(&mut server, &[pair.bob, charlie], 1);
    assert_eq!(
        extract_swings(&packets[0]).len(),
        1,
        "SWG-DST-001: tracking observer in range receives swing"
    );
    assert!(
        extract_swings(&packets[1]).is_empty(),
        "SWG-DST-001: player outside tracking range receives no swing"
    );

    server.queue(
        pair.alice,
        &ServerboundChatCommand {
            command: Bounded("execute in minecraft:the_nether run tp @s 8 64 8"),
        },
    );
    let _ = server.ticks_all(4);
    let nether = label_of(&server.app, "minecraft:the_nether");
    let (bob_nether, _) = server.join_in(nether, Uuid::from_u128(404), "BobNether");
    let _ = server.ticks_all(4);

    server.queue(pair.alice, &ServerboundPunch);
    assert!(
        extract_swings(&step_and_collect(&mut server, &[bob_nether], 1)[0]).is_empty(),
        "SWG-SRC-002: client load state must reset after dimension change"
    );
    server.queue(pair.alice, &ServerboundPlayerLoaded);
    let _ = server.ticks_all(1);
    server.queue(pair.alice, &ServerboundPunch);
    let nether_swings = extract_swings(&step_and_collect(&mut server, &[bob_nether], 1)[0]);
    assert_eq!(nether_swings.len(), 1);
    assert_ne!(
        nether_swings[0].entity_id, 0,
        "SWG-PKT-001: swing carries valid protocol entity id"
    );
}

#[test]
fn plc_block_place_swing_precedes_block_update_and_ignores_refusal() {
    let mut server = start_swing_server(&[("minecraft:overworld", "minecraft:overworld")]);
    let pair = join_pair(&mut server);
    server.queue(pair.alice, &ServerboundPlayerLoaded);
    let dim = server.dim;
    confirm_move(&mut server, pair.alice, dim, DVec3::new(8.5, 64.0, 8.5));
    confirm_move(&mut server, pair.bob, dim, DVec3::new(10.5, 64.0, 8.5));
    server.queue(
        pair.alice,
        &ServerboundChunkBatchReceived {
            desired_chunks_per_tick: 64.0,
        },
    );
    server.queue(
        pair.bob,
        &ServerboundChunkBatchReceived {
            desired_chunks_per_tick: 64.0,
        },
    );
    server.world().write_message(ColumnVersion {
        column: ColumnPos::new(0, 0),
        packet: Bytes::from_static(b"chunk"),
    });
    let _ = server.ticks_all(3);

    let stone = Kind(Item::Stone.id().number());
    let off_stack =
        ItemStack::new(stone, Count::new(16).unwrap()).set(InteractAnimation(SwingAnimation {
            kind: SwingKind::Stab,
            duration: 8,
        }));
    let world = server.world();
    world
        .commands()
        .spawn_stack(off_stack)
        .put_at(pair.alice_entity, 40);
    world.flush();

    let place = ServerboundUseItemOn {
        hand: Hand::Off,
        block_pos: BlockPos::new(6, 63, 8),
        face: Direction::Up,
        cursor_x: 0.5,
        cursor_y: 1.0,
        cursor_z: 0.5,
        inside_block: false,
        world_border_hit: false,
        sequence: VarInt(1),
    };
    server.queue(pair.alice, &place);
    let packets = step_and_collect(&mut server, &[pair.alice, pair.bob], 2);

    assert!(
        extract_swings(&packets[0]).is_empty(),
        "SWG-SRC-006: client-predicted block placement swing must be observer-only"
    );
    let bob_swings = extract_swings(&packets[1]);
    assert_eq!(bob_swings.len(), 1);
    assert_eq!(
        (
            bob_swings[0].hand,
            bob_swings[0].animation,
            bob_swings[0].duration
        ),
        (Hand::Off, SwingAnimationKind::Stab, 8)
    );
    let order = extract_order(&packets[1]);
    let swing_index = order.iter().position(|&kind| kind == "swing").unwrap();
    let block_index = order
        .iter()
        .position(|&kind| kind == "block_update")
        .unwrap();
    assert!(
        swing_index < block_index,
        "SWG-DST-003: swing_animation must precede block_update in {order:?}"
    );

    let _ = server.ticks_all(5);
    let out_of_reach = ServerboundUseItemOn {
        block_pos: BlockPos::new(100, 63, 100),
        sequence: VarInt(2),
        ..place
    };
    server.queue(pair.alice, &out_of_reach);
    let packets = step_and_collect(&mut server, &[pair.alice, pair.bob], 2);
    assert!(
        extract_swings(&packets[1]).is_empty(),
        "SWG-SRC-006: refused block place must emit no swing"
    );
}
