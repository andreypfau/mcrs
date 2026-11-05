//! Block breaking as the cell of the breaker hands it to the muxer. The muxer sends the packets
//! for the breaker first, then the changes of the columns to their holders, then the
//! acknowledgements, so a client that holds the column gets them in that order. The breaker
//! holds the column of each block that it breaks; the watcher is any other holder.

use bevy_app::Last;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_math::{DVec3, IVec3};
use mcrs_minecraft_block::definition::{BlockStateData, Blocks};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::block_action_ack::BlockActionAck;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine::column_streams::ColumnStreams;
use mcrs_minecraft_engine::muxer::session::SessionConnection;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, air_section};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, BlockEntityPos, InSection,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::host_app::{self, PlayServer};
use crate::mock_connection::make_mock_raw_connection;

const BREAKER: Uuid = Uuid::from_u128(1);
const STONE: &str = "minecraft:stone";
const NEAR: BlockPos = BlockPos::new(10, 64, 8);
const FAR: BlockPos = BlockPos::new(20, 64, 8);

/// What the cell handed the muxer, in the order that the muxer sends it.
enum Out {
    Packet(OutboundPlayerPacket),
    Change(ColumnChange),
    Ack(BlockActionAck),
}

#[derive(Resource, Default)]
struct Captured(Vec<Out>);

fn capture(
    mut packets: MessageReader<OutboundPlayerPacket>,
    mut changes: MessageReader<ColumnChange>,
    mut acks: MessageReader<BlockActionAck>,
    mut captured: ResMut<Captured>,
) {
    captured.0.extend(packets.read().cloned().map(Out::Packet));
    captured.0.extend(changes.read().cloned().map(Out::Change));
    captured.0.extend(acks.read().copied().map(Out::Ack));
}

#[derive(Clone, Copy)]
enum Client {
    Breaker(PlayerSession),
    Watcher,
}

struct Breaker {
    anchor: Entity,
    session: PlayerSession,
    dimension: Entity,
}

fn server() -> PlayServer {
    let mut server = PlayServer::start(host_app::make_host_app(), &[], 0);
    let dim = server.dim;
    let sub_app = host_app::sub_app(&mut server.app, dim);
    sub_app.init_resource::<Captured>();
    sub_app.add_systems(Last, capture);
    server
}

fn ticks(server: &mut PlayServer, n: usize) -> Vec<Out> {
    server.ticks_all(n);
    std::mem::take(&mut server.world().resource_mut::<Captured>().0)
}

fn start_action(pos: BlockPos, sequence: i32) -> ServerboundPlayerAction {
    action(PlayerAction::StartDestroyBlock, pos, sequence)
}

fn action(action: PlayerAction, pos: BlockPos, sequence: i32) -> ServerboundPlayerAction {
    ServerboundPlayerAction {
        action,
        pos,
        direction: Direction::Up,
        sequence: VarInt(sequence),
    }
}

fn default_state(server: &mut PlayServer, block: &str) -> BlockStateId {
    server
        .world()
        .resource::<Blocks>()
        .default_state(block)
        .unwrap()
}

fn state_of(
    server: &mut PlayServer,
    block: &str,
    wanted: impl Fn(&BlockStateData) -> bool,
) -> BlockStateId {
    let blocks = server.world().resource::<Blocks>();
    blocks
        .states()
        .find(|state| {
            blocks.owner(*state).unwrap().identifier == block && wanted(blocks.state(*state))
        })
        .unwrap()
}

fn part_state(server: &mut PlayServer, block: &str, values: &[(&str, &str)]) -> BlockStateId {
    let blocks = server.world().resource::<Blocks>();
    let entry = blocks.block(block).unwrap();
    values.iter().fold(
        blocks.default_state(block).unwrap(),
        |state, (name, value)| entry.with_text(state, name, value).unwrap(),
    )
}

fn holds_water(data: &BlockStateData) -> bool {
    data.fluid.is_some_and(|fluid| fluid.source)
}

fn join(server: &mut PlayServer, mode: GameMode) -> Breaker {
    let (anchor, _) = server.join(BREAKER, "Breaker");
    let session = server.session_of(anchor).unwrap();
    let player = server.player(anchor);
    let world = server.world();
    world.get_mut::<PlayerGameMode>(player).unwrap().0 = mode;
    let dimension = world.get::<InDimension>(player).unwrap().0;
    Breaker {
        anchor,
        session,
        dimension,
    }
}

fn add_section(server: &mut PlayServer, breaker: &Breaker, pos: BlockPos) {
    let section_pos = SectionPos::from(pos);
    let world = server.world();
    let held = world
        .get::<SectionIndex>(breaker.dimension)
        .unwrap()
        .get(section_pos);
    // The section of a ticket waits for a generation that this server never runs.
    if held.is_some_and(|section| world.get::<ChunkBlocks>(section).is_some()) {
        return;
    }
    let section = world
        .spawn((section_pos, InDimension(breaker.dimension), air_section()))
        .id();
    world
        .get_mut::<SectionIndex>(breaker.dimension)
        .unwrap()
        .insert(section_pos, section);
}

fn place(server: &mut PlayServer, breaker: &Breaker, pos: BlockPos, state: BlockStateId) {
    add_section(server, breaker, pos);
    server
        .world()
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension: breaker.dimension,
            pos,
            new_state: state,
            flags: BlockUpdateFlags::all(),
            recursion_left: 1,
        });
    ticks(server, 2);
}

/// A breaker in `mode` beside `state` at `pos`, in a column that the muxer subscribes to.
fn breaker_at(
    server: &mut PlayServer,
    mode: GameMode,
    state: BlockStateId,
    pos: BlockPos,
) -> Breaker {
    let breaker = join(server, mode);
    add_section(server, &breaker, pos);
    server
        .world()
        .entity_mut(breaker.dimension)
        .insert(ColumnStreams::subscribed_to([ColumnPos::from(pos)]));
    if state != BlockStateId::AIR {
        place(server, &breaker, pos, state);
    }
    breaker
}

fn section_entry(update: &PacketPayload, pos: BlockPos) -> Option<u16> {
    let PacketPayload::SectionBlocksUpdate(update) = update else {
        return None;
    };
    if update.chunk_pos != SectionPos::from(pos) {
        return None;
    }
    let local = LocalPos::from(pos);
    update
        .blocks
        .iter()
        .find(|entry| {
            (entry.off_x(), entry.off_y(), entry.off_z()) == (local.x(), local.y(), local.z())
        })
        .map(|entry| entry.block_state())
}

fn describe(payload: &PacketPayload, pos: BlockPos) -> Option<String> {
    if let Some(state) = section_entry(payload, pos) {
        return Some(format!("section_blocks_update {state}"));
    }
    match payload {
        PacketPayload::BlockUpdate(update) if update.block_pos == pos => {
            Some(format!("block_update {}", update.block_state_id.0))
        }
        PacketPayload::BlockDestruction(destruction) if destruction.pos == pos => Some(format!(
            "block_destruction {} {}",
            destruction.id.0, destruction.progress
        )),
        PacketPayload::LevelEvent(event) if event.pos == pos => Some(format!(
            "level_event {} {}",
            event.effect, event.effect_data
        )),
        PacketPayload::Disconnect(_) => Some("disconnect".to_owned()),
        _ => None,
    }
}

/// What `client` gets about `pos`, in order.
fn answer_for(out: &[Out], client: Client, pos: BlockPos) -> Vec<String> {
    out.iter()
        .filter_map(|out| match (out, client) {
            (Out::Packet(packet), Client::Breaker(session))
                if matches!(packet.target, PacketTarget::SinglePlayer(target) if target == session) =>
            {
                describe(&packet.data, pos)
            }
            (Out::Change(change), Client::Breaker(session)) if change.except != Some(session) => {
                describe(&change.payload, pos)
            }
            (Out::Change(change), Client::Watcher) => describe(&change.payload, pos),
            (Out::Ack(ack), Client::Breaker(session)) if ack.session == session => {
                Some(format!("ack {}", ack.sequence))
            }
            _ => None,
        })
        .collect()
}

struct Answer {
    breaker: Vec<String>,
    watcher: Vec<String>,
}

fn answers(out: &[Out], breaker: &Breaker, pos: BlockPos) -> Answer {
    Answer {
        breaker: answer_for(out, Client::Breaker(breaker.session), pos),
        watcher: answer_for(out, Client::Watcher, pos),
    }
}

fn send(server: &mut PlayServer, breaker: &Breaker, action: &ServerboundPlayerAction) -> Vec<Out> {
    server.queue(breaker.anchor, action);
    ticks(server, 2)
}

fn break_at(server: &mut PlayServer, mode: GameMode, state: BlockStateId, pos: BlockPos) -> Answer {
    let breaker = breaker_at(server, mode, state, pos);
    let out = send(server, &breaker, &start_action(pos, 5));
    answers(&out, &breaker, pos)
}

#[test]
fn brk_brk_001_the_breaker_gets_the_new_state_and_then_the_ack() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);

    let answer = break_at(&mut server, GameMode::Creative, stone, NEAR);

    assert_eq!(answer.breaker, ["block_update 0", "ack 5"]);
}

#[test]
fn brk_brk_003_an_observer_gets_the_level_event_and_then_the_new_state() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);

    let answer = break_at(&mut server, GameMode::Creative, stone, NEAR);

    assert_eq!(
        answer.watcher,
        [
            format!("level_event 2001 {}", stone.number()),
            "block_update 0".to_owned()
        ]
    );
}

fn assert_refused_with_the_state(mode: GameMode) {
    let mut server = server();
    let stone = default_state(&mut server, STONE);

    let answer = break_at(&mut server, mode, stone, NEAR);

    assert_eq!(
        answer.breaker,
        [
            format!("block_update {}", stone.number()),
            "ack 5".to_owned()
        ]
    );
    assert_eq!(answer.watcher, Vec::<String>::new());
}

#[test]
fn brk_ref_001_adventure_mode_gets_the_current_state_and_then_the_ack() {
    assert_refused_with_the_state(GameMode::Adventure);
}

#[test]
fn brk_ref_001_spectator_mode_gets_the_current_state_and_then_the_ack() {
    assert_refused_with_the_state(GameMode::Spectator);
}

#[test]
fn brk_ord_001_reach_comes_before_the_game_mode() {
    for mode in [GameMode::Creative, GameMode::Adventure, GameMode::Spectator] {
        let mut server = server();
        let stone = default_state(&mut server, STONE);

        let answer = break_at(&mut server, mode, stone, FAR);

        assert_eq!(answer.breaker, ["ack 5"], "{mode:?}");
        assert_eq!(answer.watcher, Vec::<String>::new(), "{mode:?}");
    }
}

#[test]
fn brk_ord_001_an_abort_out_of_reach_gets_only_the_ack() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let breaker = breaker_at(&mut server, GameMode::Creative, stone, FAR);

    let out = send(
        &mut server,
        &breaker,
        &action(PlayerAction::AbortDestroyBlock, FAR, 5),
    );

    let answer = answers(&out, &breaker, FAR);
    assert_eq!(answer.breaker, ["ack 5"]);
    assert_eq!(answer.watcher, Vec::<String>::new());
}

#[test]
fn brk_ack_002_air_gets_only_the_ack_and_an_observer_gets_a_level_event() {
    let answer = break_at(&mut server(), GameMode::Creative, BlockStateId::AIR, NEAR);

    assert_eq!(answer.breaker, ["ack 5"]);
    assert_eq!(answer.watcher, ["level_event 2001 0"]);
}

fn assert_fluid_stays(server: &mut PlayServer, fluid: BlockStateId) {
    let answer = break_at(server, GameMode::Creative, fluid, NEAR);

    assert_eq!(answer.breaker, ["ack 5"], "{fluid:?}");
    assert_eq!(
        answer.watcher,
        [format!("level_event 2001 {}", fluid.number())],
        "{fluid:?}"
    );
}

#[test]
fn brk_ack_004_a_fluid_at_a_level_up_to_8_stays() {
    let flowing = |data: &BlockStateData| data.fluid.is_some_and(|fluid| !fluid.source);
    for block in ["minecraft:water", "minecraft:lava"] {
        let mut server = server();
        let source = state_of(&mut server, block, holds_water);
        let level_8 = part_state(&mut server, block, &[("level", "8")]);
        let flowing = state_of(&mut server, block, flowing);
        for fluid in [source, flowing, level_8] {
            assert_fluid_stays(&mut self::server(), fluid);
        }
    }
}

#[test]
fn brk_ack_005_a_fluid_above_level_8_changes_to_level_8() {
    for block in ["minecraft:water", "minecraft:lava"] {
        for level in ["9", "15"] {
            let mut server = server();
            let old = part_state(&mut server, block, &[("level", level)]);
            let new = part_state(&mut server, block, &[("level", "8")]);

            let answer = break_at(&mut server, GameMode::Creative, old, NEAR);

            let update = format!("block_update {}", new.number());
            assert_eq!(
                answer.breaker,
                [update.clone(), "ack 5".to_owned()],
                "{block} {level}"
            );
            assert_eq!(
                answer.watcher,
                [format!("level_event 2001 {}", old.number()), update],
                "{block} {level}"
            );
        }
    }
}

#[test]
fn brk_ack_003_a_block_that_holds_water_changes_to_a_water_source() {
    for block in [
        "minecraft:oak_slab",
        "minecraft:oak_stairs",
        "minecraft:oak_fence",
        "minecraft:sea_pickle",
        "minecraft:ladder",
        "minecraft:kelp",
        "minecraft:kelp_plant",
        "minecraft:seagrass",
        "minecraft:bubble_column",
    ] {
        let mut server = server();
        let old = state_of(&mut server, block, holds_water);
        let water = default_state(&mut server, "minecraft:water");

        let answer = break_at(&mut server, GameMode::Creative, old, NEAR);

        let update = format!("block_update {}", water.number());
        assert_eq!(
            answer.breaker,
            [update.clone(), "ack 5".to_owned()],
            "{block}"
        );
        assert_eq!(
            answer.watcher,
            [format!("level_event 2001 {}", old.number()), update],
            "{block}"
        );
    }
}

#[test]
fn brk_ack_003_the_same_block_with_no_water_changes_to_air() {
    let mut server = server();
    let dry = state_of(&mut server, "minecraft:oak_slab", |data| {
        data.fluid.is_none()
    });

    let answer = break_at(&mut server, GameMode::Creative, dry, NEAR);

    assert_eq!(answer.breaker, ["block_update 0", "ack 5"]);
}

fn answer_to(server: &mut PlayServer, kind: PlayerAction) -> (Breaker, Answer) {
    let stone = default_state(server, STONE);
    let breaker = breaker_at(server, GameMode::Creative, stone, NEAR);
    let out = send(server, &breaker, &action(kind, NEAR, 5));
    let answer = answers(&out, &breaker, NEAR);
    (breaker, answer)
}

#[test]
fn brk_req_002_a_change_and_a_stop_get_only_the_ack() {
    for kind in [
        PlayerAction::ChangeDestroyDirection,
        PlayerAction::StopDestroyBlock,
    ] {
        let (_, answer) = answer_to(&mut server(), kind);

        assert_eq!(answer.breaker, ["ack 5"], "{kind:?}");
        assert_eq!(answer.watcher, Vec::<String>::new(), "{kind:?}");
    }
}

#[test]
fn brk_req_002_an_abort_gets_the_ack_and_an_observer_gets_a_destruction_of_255() {
    let (breaker, answer) = answer_to(&mut server(), PlayerAction::AbortDestroyBlock);

    assert_eq!(answer.breaker, ["ack 5"]);
    assert_eq!(
        answer.watcher,
        [format!(
            "block_destruction {} -1",
            player_network_id(breaker.session)
        )]
    );
}

#[test]
fn brk_brk_009_a_start_action_extinguishes_fire() {
    for block in ["minecraft:fire", "minecraft:soul_fire"] {
        let mut server = server();
        let fire = default_state(&mut server, block);

        let answer = break_at(&mut server, GameMode::Creative, fire, NEAR);

        assert_eq!(
            answer.breaker,
            ["level_event 1009 0", "block_update 0", "ack 5"],
            "{block}"
        );
        assert_eq!(
            answer.watcher,
            ["level_event 1009 0", "block_update 0"],
            "{block}"
        );
    }
}

fn breaker_beside(
    server: &mut PlayServer,
    mode: GameMode,
    y: impl Fn(&DimensionTypeConfig) -> i32,
) -> (Breaker, BlockPos) {
    let breaker = join(server, mode);
    let player = server.player(breaker.anchor);
    let world = server.world();
    let pos = BlockPos::new(10, y(world.get(breaker.dimension).unwrap()), 8);
    world.get_mut::<Transform>(player).unwrap().translation =
        pos.as_dvec3() + DVec3::new(0.5, 2.0, 0.5);
    (breaker, pos)
}

fn above_the_world(config: &DimensionTypeConfig) -> i32 {
    config.min_y + config.height as i32
}

fn below_the_world(config: &DimensionTypeConfig) -> i32 {
    config.min_y - 1
}

#[test]
fn brk_ref_001_each_action_above_the_height_limit_gets_void_air_and_then_the_ack() {
    for kind in [
        PlayerAction::StartDestroyBlock,
        PlayerAction::ChangeDestroyDirection,
        PlayerAction::AbortDestroyBlock,
        PlayerAction::StopDestroyBlock,
    ] {
        let mut server = server();
        let void_air = default_state(&mut server, "minecraft:void_air");
        let (breaker, pos) = breaker_beside(&mut server, GameMode::Creative, above_the_world);

        let answer = answers(
            &send(&mut server, &breaker, &action(kind, pos, 5)),
            &breaker,
            pos,
        );

        assert_eq!(
            answer.breaker,
            [
                format!("block_update {}", void_air.number()),
                "ack 5".to_owned()
            ],
            "{kind:?}"
        );
        assert_eq!(answer.watcher, Vec::<String>::new(), "{kind:?}");
    }
}

#[test]
fn brk_ord_002_below_the_height_limit_an_observer_gets_a_level_event_of_void_air() {
    let mut server = server();
    let void_air = default_state(&mut server, "minecraft:void_air");
    let (breaker, pos) = breaker_beside(&mut server, GameMode::Creative, below_the_world);

    let out = send(&mut server, &breaker, &start_action(pos, 5));

    let answer = answers(&out, &breaker, pos);
    assert_eq!(answer.breaker, ["ack 5"]);
    assert_eq!(
        answer.watcher,
        [format!("level_event 2001 {}", void_air.number())]
    );
    assert!(
        !out.iter().any(|out| matches!(out, Out::Change(change)
            if matches!(change.payload, PacketPayload::BlockUpdate(_)))),
        "a position below the world changes nothing"
    );
}

#[test]
fn brk_ord_001_below_the_height_limit_adventure_mode_gets_void_air() {
    let mut server = server();
    let void_air = default_state(&mut server, "minecraft:void_air");
    let (breaker, pos) = breaker_beside(&mut server, GameMode::Adventure, below_the_world);

    let answer = answers(
        &send(&mut server, &breaker, &start_action(pos, 5)),
        &breaker,
        pos,
    );

    assert_eq!(
        answer.breaker,
        [
            format!("block_update {}", void_air.number()),
            "ack 5".to_owned()
        ]
    );
    assert_eq!(answer.watcher, Vec::<String>::new());
}

#[test]
fn brk_seq_005_a_negative_sequence_breaks_the_block_and_closes_the_connection() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let breaker = breaker_at(&mut server, GameMode::Creative, stone, NEAR);
    let (raw, mut sent) = make_mock_raw_connection();
    let world = server.app.world_mut();
    let connection = world
        .get::<SessionConnection>(breaker.anchor)
        .unwrap()
        .entity();
    world
        .entity_mut(connection)
        .insert(ServerSideConnection { raw: Box::new(raw) });

    let answer = answers(
        &send(&mut server, &breaker, &start_action(NEAR, -1)),
        &breaker,
        NEAR,
    );

    assert_eq!(answer.breaker, ["disconnect", "block_update 0"]);
    assert_eq!(
        answer.watcher,
        [
            format!("level_event 2001 {}", stone.number()),
            "block_update 0".to_owned()
        ]
    );
    let world = server.app.world();
    assert!(world.get::<ServerSideConnection>(connection).is_none());
    let blob = sent.try_recv().expect("the disconnect packet");
    let key = b"disconnect.packetError";
    assert!(
        blob.windows(key.len()).any(|window| window == key),
        "{blob:?}"
    );
}

#[test]
fn a_stop_in_adventure_mode_does_not_send_the_block() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let breaker = breaker_at(&mut server, GameMode::Adventure, stone, NEAR);

    let out = send(
        &mut server,
        &breaker,
        &action(PlayerAction::StopDestroyBlock, NEAR, 5),
    );

    assert_eq!(answers(&out, &breaker, NEAR).breaker, ["ack 5"]);
}

#[test]
fn brk_seq_003_a_start_a_stop_and_an_abort_in_one_tick_break_with_one_ack() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let breaker = breaker_at(&mut server, GameMode::Creative, stone, NEAR);

    server.queue(
        breaker.anchor,
        &action(PlayerAction::StartDestroyBlock, NEAR, 8),
    );
    server.queue(
        breaker.anchor,
        &action(PlayerAction::StopDestroyBlock, NEAR, 9),
    );
    server.queue(
        breaker.anchor,
        &action(PlayerAction::AbortDestroyBlock, NEAR, 10),
    );

    assert_eq!(
        answers(&ticks(&mut server, 2), &breaker, NEAR).breaker,
        ["block_update 0", "ack 10"]
    );
}

#[test]
fn brk_seq_003_the_same_start_three_times_in_one_tick_breaks_the_block_once() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let breaker = breaker_at(&mut server, GameMode::Creative, stone, NEAR);

    for sequence in [3, 4, 5] {
        server.queue(breaker.anchor, &start_action(NEAR, sequence));
    }

    let answer = answers(&ticks(&mut server, 2), &breaker, NEAR);
    assert_eq!(answer.breaker, ["block_update 0", "ack 5"]);
    assert_eq!(
        answer.watcher,
        [
            format!("level_event 2001 {}", stone.number()),
            "level_event 2001 0".to_owned(),
            "level_event 2001 0".to_owned(),
            "block_update 0".to_owned()
        ],
        "a later start of the tick finds the air that the first one left"
    );
}

#[test]
fn brk_seq_001_requests_of_one_tick_get_one_ack_with_the_highest_sequence() {
    let mut server = server();
    let breaker = join(&mut server, GameMode::Creative);

    server.queue(breaker.anchor, &start_action(BlockPos::new(2, 0, 0), 31));
    server.queue(breaker.anchor, &start_action(BlockPos::new(2, 0, 1), 30));

    let acks: Vec<i32> = ticks(&mut server, 2)
        .iter()
        .filter_map(|out| match out {
            Out::Ack(ack) => Some(ack.sequence),
            _ => None,
        })
        .collect();
    assert_eq!(acks, [31]);
}

fn level_events(out: &[Out], client: Client) -> Vec<(BlockPos, i32)> {
    out.iter()
        .filter_map(|out| match (out, client) {
            (Out::Packet(packet), Client::Breaker(session))
                if matches!(packet.target, PacketTarget::SinglePlayer(target) if target == session) =>
            {
                Some(&packet.data)
            }
            (Out::Change(change), Client::Breaker(session)) if change.except != Some(session) => {
                Some(&change.payload)
            }
            (Out::Change(change), Client::Watcher) => Some(&change.payload),
            _ => None,
        })
        .filter_map(|payload| match payload {
            PacketPayload::LevelEvent(event) => Some((event.pos, event.effect_data)),
            _ => None,
        })
        .collect()
}

struct Part {
    pos: BlockPos,
    state: BlockStateId,
}

fn assert_pair_breaks(
    server: &mut PlayServer,
    main: Part,
    other: Part,
    hit_main: bool,
    after: BlockStateId,
) {
    let breaker = breaker_at(server, GameMode::Creative, main.state, main.pos);
    place(server, &breaker, other.pos, other.state);
    let hit = if hit_main { main.pos } else { other.pos };

    let out = send(server, &breaker, &start_action(hit, 5));

    let update = format!("section_blocks_update {}", after.number());
    let ack = "ack 5".to_owned();
    let main_event = (main.pos, main.state.number() as i32);
    let other_event = (other.pos, other.state.number() as i32);
    assert_eq!(
        level_events(&out, Client::Breaker(breaker.session)),
        [main_event],
        "the breaker gets the event of the main part only"
    );
    assert_eq!(
        answer_for(&out, Client::Breaker(breaker.session), other.pos),
        [update.clone(), ack]
    );

    let order = if hit_main {
        vec![main_event, other_event, main_event]
    } else {
        vec![other_event, main_event]
    };
    assert_eq!(level_events(&out, Client::Watcher), order);
    assert_eq!(
        answer_for(&out, Client::Watcher, other.pos),
        [format!("level_event 2001 {}", other.state.number()), update]
    );
}

fn upper_and_lower(server: &mut PlayServer, block: &str) -> (Part, Part) {
    (
        Part {
            pos: NEAR + IVec3::Y,
            state: part_state(server, block, &[("half", "upper")]),
        },
        Part {
            pos: NEAR,
            state: part_state(server, block, &[("half", "lower")]),
        },
    )
}

#[test]
fn brk_prt_001_breaking_either_half_of_a_tall_block_removes_both() {
    for block in [
        "minecraft:tall_grass",
        "minecraft:sunflower",
        "minecraft:oak_door",
        "minecraft:small_dripleaf",
    ] {
        for hit_main in [true, false] {
            let mut server = server();
            let (main, other) = upper_and_lower(&mut server, block);
            assert_pair_breaks(&mut server, main, other, hit_main, BlockStateId::AIR);
        }
    }
}

#[test]
fn brk_prt_001_breaking_either_part_of_a_bed_removes_both() {
    for (facing, offset) in [
        ("south", IVec3::Z),
        ("north", IVec3::NEG_Z),
        ("east", IVec3::X),
        ("west", IVec3::NEG_X),
    ] {
        for hit_main in [true, false] {
            let mut server = server();
            let foot = part_state(
                &mut server,
                "minecraft:red_bed",
                &[("part", "foot"), ("facing", facing)],
            );
            let head = part_state(
                &mut server,
                "minecraft:red_bed",
                &[("part", "head"), ("facing", facing)],
            );
            assert_pair_breaks(
                &mut server,
                Part {
                    pos: NEAR,
                    state: foot,
                },
                Part {
                    pos: NEAR + offset,
                    state: head,
                },
                hit_main,
                BlockStateId::AIR,
            );
        }
    }
}

#[test]
fn brk_ack_003_both_halves_of_tall_seagrass_become_water() {
    for hit_main in [true, false] {
        let mut server = server();
        let water = default_state(&mut server, "minecraft:water");
        let (main, other) = upper_and_lower(&mut server, "minecraft:tall_seagrass");
        assert_pair_breaks(&mut server, main, other, hit_main, water);
    }
}

#[test]
fn brk_prt_001_a_half_alone_does_not_take_the_block_next_to_it() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let upper = part_state(&mut server, "minecraft:tall_grass", &[("half", "upper")]);
    let above = NEAR + IVec3::Y;
    let breaker = breaker_at(&mut server, GameMode::Creative, stone, NEAR);
    place(&mut server, &breaker, above, upper);

    let out = send(&mut server, &breaker, &start_action(above, 5));

    assert_eq!(answers(&out, &breaker, NEAR).breaker, ["ack 5"]);
}

#[test]
fn brk_prt_001_a_half_of_another_block_is_left_alone() {
    let mut server = server();
    let door = part_state(&mut server, "minecraft:oak_door", &[("half", "lower")]);
    let grass = part_state(&mut server, "minecraft:tall_grass", &[("half", "upper")]);
    let above = NEAR + IVec3::Y;
    let breaker = breaker_at(&mut server, GameMode::Creative, door, NEAR);
    place(&mut server, &breaker, above, grass);

    let out = send(&mut server, &breaker, &start_action(above, 5));

    assert_eq!(answers(&out, &breaker, NEAR).breaker, ["ack 5"]);
}

#[test]
fn a_stair_with_a_half_property_is_one_block() {
    let mut server = server();
    let stairs = part_state(&mut server, "minecraft:oak_stairs", &[("half", "top")]);
    let stone = default_state(&mut server, STONE);
    let above = NEAR + IVec3::Y;
    let below = NEAR - IVec3::Y;
    let breaker = breaker_at(&mut server, GameMode::Creative, stairs, NEAR);
    place(&mut server, &breaker, above, stone);
    place(&mut server, &breaker, below, stone);

    let out = send(&mut server, &breaker, &start_action(NEAR, 5));

    for pos in [above, below] {
        assert_eq!(answers(&out, &breaker, pos).breaker, ["ack 5"]);
    }
}

#[test]
fn a_pair_across_two_sections_gets_one_block_update_for_each() {
    let upper_pos = BlockPos::new(10, 64, 8);
    let lower_pos = BlockPos::new(10, 63, 8);
    let mut server = server();
    let upper = part_state(&mut server, "minecraft:oak_door", &[("half", "upper")]);
    let lower = part_state(&mut server, "minecraft:oak_door", &[("half", "lower")]);
    let breaker = breaker_at(&mut server, GameMode::Creative, upper, upper_pos);
    place(&mut server, &breaker, lower_pos, lower);

    let out = send(&mut server, &breaker, &start_action(lower_pos, 5));

    assert_eq!(
        answers(&out, &breaker, upper_pos).breaker,
        [
            format!("level_event 2001 {}", upper.number()),
            "block_update 0".to_owned(),
            "ack 5".to_owned()
        ]
    );
    assert_eq!(
        answers(&out, &breaker, lower_pos).breaker,
        ["block_update 0", "ack 5"]
    );
}

#[test]
fn blocks_of_one_section_that_change_to_different_states_carry_each_state() {
    let mut server = server();
    let stone = default_state(&mut server, STONE);
    let dirt = default_state(&mut server, "minecraft:dirt");
    let breaker = breaker_at(&mut server, GameMode::Creative, BlockStateId::AIR, NEAR);
    let other = NEAR + IVec3::X;
    let mut requests = server.world().resource_mut::<Messages<BlockSetRequest>>();
    for (pos, state) in [(NEAR, stone), (other, dirt)] {
        requests.write(BlockSetRequest {
            dimension: breaker.dimension,
            pos,
            new_state: state,
            flags: BlockUpdateFlags::all(),
            recursion_left: 1,
        });
    }

    let out = ticks(&mut server, 2);

    assert_eq!(
        answers(&out, &breaker, NEAR).watcher,
        [format!("section_blocks_update {}", stone.number())]
    );
    assert_eq!(
        answers(&out, &breaker, other).watcher,
        [format!("section_blocks_update {}", dirt.number())]
    );
}

fn block_entity_at(server: &mut PlayServer, breaker: &Breaker, pos: BlockPos) -> Entity {
    let world = server.world();
    let section = world
        .get::<SectionIndex>(breaker.dimension)
        .unwrap()
        .get(SectionPos::from(pos))
        .unwrap();
    world
        .spawn((
            BlockEntityPos(pos),
            InDimension(breaker.dimension),
            BlockEntity(GeneratedBlockEntity::chest(pos, "loot".to_owned(), 0)),
            InSection(section),
        ))
        .id()
}

fn exists(server: &mut PlayServer, entity: Entity) -> bool {
    server.world().get_entity(entity).is_ok()
}

#[test]
fn a_broken_block_takes_only_its_own_block_entity() {
    let mut server = server();
    let chest = default_state(&mut server, "minecraft:chest");
    let breaker = breaker_at(&mut server, GameMode::Creative, chest, NEAR);
    let other = NEAR + IVec3::X;
    place(&mut server, &breaker, other, chest);
    let broken = block_entity_at(&mut server, &breaker, NEAR);
    let kept = block_entity_at(&mut server, &breaker, other);

    send(&mut server, &breaker, &start_action(NEAR, 5));

    assert!(!exists(&mut server, broken));
    assert!(exists(&mut server, kept));
}

#[test]
fn a_broken_bed_takes_the_block_entity_of_both_parts() {
    let mut server = server();
    let foot = part_state(
        &mut server,
        "minecraft:red_bed",
        &[("part", "foot"), ("facing", "south")],
    );
    let head = part_state(
        &mut server,
        "minecraft:red_bed",
        &[("part", "head"), ("facing", "south")],
    );
    let breaker = breaker_at(&mut server, GameMode::Creative, foot, NEAR);
    let head_pos = NEAR + IVec3::Z;
    place(&mut server, &breaker, head_pos, head);
    let of_foot = block_entity_at(&mut server, &breaker, NEAR);
    let of_head = block_entity_at(&mut server, &breaker, head_pos);

    send(&mut server, &breaker, &start_action(NEAR, 5));

    assert!(!exists(&mut server, of_foot));
    assert!(!exists(&mut server, of_head));
}

#[test]
fn a_new_state_of_the_same_block_keeps_the_block_entity() {
    let mut server = server();
    let north = part_state(&mut server, "minecraft:chest", &[("facing", "north")]);
    let east = part_state(&mut server, "minecraft:chest", &[("facing", "east")]);
    let breaker = breaker_at(&mut server, GameMode::Creative, north, NEAR);
    let block_entity = block_entity_at(&mut server, &breaker, NEAR);

    place(&mut server, &breaker, NEAR, east);

    assert!(exists(&mut server, block_entity));
}
