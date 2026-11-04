use bevy_app::{App, Last};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Entity, IntoScheduleConfigs, ResMut, Resource};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine::between_ticks::drain_dim_spawn_queue;
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_engine::column_view::ColumnView;
use mcrs_minecraft_engine::coordinator::dim::DimSubAppHandle;
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, SectionStates};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::lifecycle::stage::{SectionStage, SectionStageChanged};
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLightUpdate;
use mcrs_minecraft_protocol_game::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_light::emit_light_updates;
use mcrs_minecraft_vanilla_light::{LightBudget, LightLink};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::host_app;

const SECTIONS: std::ops::Range<i32> = -4..20;
const STONE_SECTION_Y: i32 = 0;

fn state(blocks: &Blocks, name: &str) -> BlockStateId {
    blocks
        .block(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
        .default_state_id
}

fn filled(block: BlockStateId) -> SectionStates {
    SectionStates::homogeneous(block)
}

/// Spawn one column of section entities exactly as `integrate_generated_columns`
/// does, then pump until the engine has nothing left to do.
fn light_one_column(app: &mut App, label: Entity, stone_floor: bool) {
    let blocks = app.world().resource::<Blocks>().clone();
    let air = state(&blocks, "minecraft:air");
    let stone = state(&blocks, "minecraft:stone");

    let sub_app = host_app::sub_app(app, label);
    let world = sub_app.world_mut();
    let dimension = world
        .query_filtered::<Entity, bevy_ecs::prelude::With<ColumnIndex>>()
        .iter(world)
        .next()
        .expect("the dimension entity");
    let config = *world
        .query::<&DimensionTypeConfig>()
        .single(world)
        .expect("the dimension's shape");
    let min_section = config.min_y >> 4;
    for y in min_section..min_section + config.section_count as i32 {
        let solid = stone_floor && y == STONE_SECTION_Y;
        let pos = SectionPos::new(0, y, 0);
        let section = world
            .spawn((
                pos,
                InDimension(dimension),
                ChunkBlocks::new(filled(if solid { stone } else { air })),
                SectionStage::Loaded,
            ))
            .id();
        world.write_message(SectionStageChanged::spawned(
            section,
            pos,
            dimension,
            SectionStage::Loaded,
        ));
    }
    settle(app, label);
}

/// Updates until an update leaves the light stage owing nothing, and waits for the stage to
/// answer between updates. A published answer can raise more light work, such as a new surface.
fn settle(app: &mut App, label: Entity) {
    for _ in 0..400 {
        app.update();
        let world = host_app::sub_app(app, label).world_mut();
        let mut light = world.resource_mut::<LightLink>();
        if light.is_idle() {
            return;
        }
        bevy_tasks::block_on(light.answered());
    }
    panic!("light never settled");
}

fn published(app: &mut App, label: Entity, pos: SectionPos) -> (BlockLight, SkyLight) {
    let world = host_app::sub_app(app, label).world_mut();
    let mut sections = world.query::<(&SectionPos, &BlockLight, &SkyLight)>();
    let (_, block, sky) = sections
        .iter(world)
        .find(|(at, _, _)| **at == pos)
        .unwrap_or_else(|| panic!("{pos:?} was published"));
    (block.clone(), sky.clone())
}

fn spawn_dimension(id: &str, dimension_type: &str) -> (App, Entity) {
    let mut app = host_app::make_host_app();
    host_app::enable_lighting(&mut app);
    host_app::enqueue_spawn(&mut app, id, dimension_type);
    drain_dim_spawn_queue(&mut app);

    let mut handles = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handle = handles
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("one sub-app handle");
    (app, handle)
}

/// Stands in for `apply_voxel_set_requests`, which writes the palette and then
/// the message; the chunk index it resolves through is not wired here.
fn place_torch(app: &mut App, label: Entity, at: BlockPos) -> u8 {
    let blocks = app.world().resource::<Blocks>().clone();
    let torch = state(&blocks, "minecraft:torch");
    let emission = blocks
        .state(blocks.default_state("minecraft:torch").unwrap())
        .light_emission;

    let sub_app = host_app::sub_app(app, label);
    let chunk_pos = SectionPos::from(at);
    let mut sections = sub_app.world_mut().query::<(Entity, &SectionPos)>();
    let chunk = sections
        .iter(sub_app.world())
        .find(|(_, pos)| **pos == chunk_pos)
        .map(|(entity, _)| entity)
        .expect("the section holding the torch");
    sub_app
        .world_mut()
        .get_mut::<ChunkBlocks>(chunk)
        .expect("the section has blocks")
        .make_mut()
        .0
        .set(
            (at.x & 15) as usize,
            (at.y & 15) as usize,
            (at.z & 15) as usize,
            torch,
        );
    sub_app.world_mut().write_message(BlockPlaced {
        chunk,
        chunk_pos,
        block_pos: at,
        old_state: state(&blocks, "minecraft:air"),
        new_state: torch,
        flags: BlockUpdateFlags::empty(),
    });
    settle(app, label);
    emission
}

fn a_placed_torch_lights_its_neighbourhood() {
    let (mut app, label) = spawn_dimension("minecraft:overworld", "minecraft:overworld");
    light_one_column(&mut app, label, true);

    let at = BlockPos::new(8, -8, 8);
    let emission = place_torch(&mut app, label, at);
    assert!(emission > 1, "a torch emits light");

    let chunk_pos = SectionPos::from(at);
    let (block, _) = published(&mut app, label, chunk_pos);
    assert_eq!(block.0.get(8, 8, 8), emission);
    assert_eq!(block.0.get(9, 8, 8), emission - 1);
    assert_eq!(block.0.get(8, 8, 10), emission - 2);
}

fn a_dimension_without_a_sky_publishes_none() {
    let (mut app, label) = spawn_dimension("minecraft:the_nether", "minecraft:the_nether");
    light_one_column(&mut app, label, false);

    for y in [15, 0] {
        let (_, sky) = published(&mut app, label, SectionPos::new(0, y, 0));
        for local_y in [0, 8, 15] {
            assert_eq!(
                sky.0.get(8, local_y, 8),
                0,
                "section {y} has no sky to let in"
            );
        }
    }
}

#[derive(Resource, Default)]
struct CapturedLightUpdates(Vec<OutboundPlayerPacket>);

/// Reads the dim's outbox in the same schedule the emitter writes it, before the output of the
/// tick drains it.
fn capture_light_updates(
    mut reader: MessageReader<OutboundPlayerPacket>,
    mut captured: ResMut<CapturedLightUpdates>,
) {
    for packet in reader.read() {
        if matches!(packet.data, PacketPayload::LightUpdate(_)) {
            captured.0.push(packet.clone());
        }
    }
}

const WIRE_ROWS: usize = 26;
fn torch_at() -> BlockPos {
    BlockPos::new(8, -8, 8)
}

fn wire_row(section_y: i32) -> usize {
    (section_y - SECTIONS.start + 1) as usize
}

fn nibble(
    chunk: &mcrs_minecraft_protocol_game::chunk::LightChunk,
    x: usize,
    y: usize,
    z: usize,
) -> u8 {
    let idx = (y << 8) | (z << 4) | x;
    (chunk.0[idx >> 1] >> ((idx & 1) * 4)) & 0x0F
}

/// Light one column, give it a viewing player that has (or has not) already
/// received it, then place a torch and collect what the emitter sent.
///
/// Returns the player's session, because that is how the bus addresses a
/// player: the dimension world's own player entity resolves to nothing on the host.
fn torch_delta(already_sent: bool) -> (Vec<OutboundPlayerPacket>, PlayerSession) {
    let (mut app, label) = spawn_dimension("minecraft:overworld", "minecraft:overworld");
    let sub_app = host_app::sub_app(&mut app, label);
    sub_app.init_resource::<CapturedLightUpdates>();
    sub_app.add_systems(Last, capture_light_updates.after(emit_light_updates));

    light_one_column(&mut app, label, true);

    let column = ColumnPos::new(0, 0);
    let session = PlayerSession(1);
    let sub_app = host_app::sub_app(&mut app, label);
    let world = sub_app.world_mut();
    let dimension = world
        .query_filtered::<Entity, bevy_ecs::prelude::With<ColumnIndex>>()
        .iter(world)
        .next()
        .expect("the dimension entity");
    let view = if already_sent {
        ColumnView::holding([column])
    } else {
        ColumnView::default()
    };
    world.spawn((Player, view, Owner(session), InDimension(dimension)));
    world.resource_mut::<CapturedLightUpdates>().0.clear();

    place_torch(&mut app, label, torch_at());

    let captured = host_app::sub_app(&mut app, label)
        .world()
        .resource::<CapturedLightUpdates>()
        .0
        .clone();
    (captured, session)
}

/// A player that holds the column is sent every correction to its light, standing
/// still or not.
fn a_torch_sends_one_delta_carrying_only_the_rows_it_changed() {
    let (captured, session) = torch_delta(true);

    assert_eq!(captured.len(), 1, "one packet for one column");
    let packet = &captured[0];
    match &packet.target {
        PacketTarget::PlayerSet(set) => assert_eq!(set.as_slice(), [session]),
        other => panic!("expected a player set, got {other:?}"),
    }
    assert_eq!(
        packet.priority,
        PacketPriority::High,
        "a column's light is sent in full once, so a shed delta is never made good"
    );
    let PacketPayload::LightUpdate(ClientboundLightUpdate { x, z, light_data }) = &packet.data
    else {
        unreachable!("filtered above");
    };
    assert_eq!((x.0, z.0), (0, 0));

    let rows = unpack_light_data(light_data, WIRE_ROWS).expect("the delta decodes");
    let torch_row = wire_row(SectionPos::from(torch_at()).y);
    match &rows.block[torch_row] {
        RowLight::Filled(chunk) => {
            assert_eq!(nibble(chunk, 8, 8, 8), 14, "the torch cell");
            assert_eq!(nibble(chunk, 9, 8, 8), 13, "one step away");
            assert_eq!(nibble(chunk, 8, 8, 10), 12, "two steps away");
        }
        other => panic!("the torch's own section is not filled: {other:?}"),
    }
    assert_eq!(
        rows.block[wire_row(10)],
        RowLight::Unchanged,
        "a section the torch cannot reach"
    );
    assert_eq!(
        rows.block[0],
        RowLight::Unchanged,
        "a delta never synthesizes the padding rows"
    );
    assert_eq!(rows.block[WIRE_ROWS - 1], RowLight::Unchanged);
    assert!(
        rows.sky.iter().all(|row| *row == RowLight::Unchanged),
        "a torch changes no sky light"
    );
}

fn a_column_the_player_never_received_gets_no_delta() {
    let (captured, _) = torch_delta(false);
    assert!(
        captured.is_empty(),
        "a delta for an unsent column is meaningless: {captured:?}"
    );
}

/// Columns two apart so no field reaches its neighbour: a job publishes every
/// loaded section of its field, and adjacent columns would be lit by each
/// other's halo rather than in the order the queue handed them out.
const SPREAD: [(i32, i32); 6] = [(0, 0), (2, 0), (0, 2), (4, 0), (0, 4), (4, 4)];

fn the_column_under_the_player_is_lit_before_the_far_ones() {
    let (mut app, label) = spawn_dimension("minecraft:overworld", "minecraft:overworld");
    let blocks = app.world().resource::<Blocks>().clone();
    let stone = state(&blocks, "minecraft:stone");
    let air = state(&blocks, "minecraft:air");

    let sub_app = host_app::sub_app(&mut app, label);
    // One column per epoch, so the order the queue hands work out is the order
    // light is published in.
    sub_app.insert_resource(LightBudget {
        cells_per_epoch: 1,
        epochs_in_flight: 1,
    });
    let world = sub_app.world_mut();
    let dimension = world
        .query_filtered::<Entity, bevy_ecs::prelude::With<ColumnIndex>>()
        .iter(world)
        .next()
        .expect("the dimension entity");
    world.spawn((Player, Transform::from_xyz(8.0, 64.0, 8.0)));
    for (x, z) in SPREAD {
        for y in SECTIONS {
            let solid = y == STONE_SECTION_Y;
            let pos = SectionPos::new(x, y, z);
            let section = world
                .spawn((
                    pos,
                    InDimension(dimension),
                    ChunkBlocks::new(filled(if solid { stone } else { air })),
                    SectionStage::Loaded,
                ))
                .id();
            world.write_message(SectionStageChanged::spawned(
                section,
                pos,
                dimension,
                SectionStage::Loaded,
            ));
        }
    }

    let mut lit: Vec<(ColumnPos, usize)> = Vec::new();
    for tick in 0..400 {
        app.update();
        let world = host_app::sub_app(&mut app, label).world_mut();
        if !world.resource::<LightLink>().is_idle() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let newly: Vec<ColumnPos> = world
            .query_filtered::<&SectionPos, bevy_ecs::prelude::With<BlockLight>>()
            .iter(world)
            .map(|pos| ColumnPos::from(*pos))
            .filter(|column| !lit.iter().any(|(seen, _)| seen == column))
            .collect();
        for column in newly {
            if !lit.iter().any(|(seen, _)| *seen == column) {
                lit.push((column, tick));
            }
        }
        if lit.len() == SPREAD.len() {
            break;
        }
    }

    assert_eq!(
        lit.len(),
        SPREAD.len(),
        "every column was lit eventually, got {lit:?}"
    );
    let origin = ColumnPos::new(0, 0);
    let mut by_distance: Vec<(i32, usize, ColumnPos)> = lit
        .iter()
        .map(|(column, tick)| (column.distance_squared(origin), *tick, *column))
        .collect();
    by_distance.sort();
    assert!(
        by_distance.windows(2).all(|pair| pair[0].1 <= pair[1].1),
        "a nearer column is never lit later than a farther one, got {by_distance:?}"
    );
}

#[test]
fn the_dimension_lights_its_columns_and_sends_what_changed() {
    a_placed_torch_lights_its_neighbourhood();
    a_dimension_without_a_sky_publishes_none();
    a_torch_sends_one_delta_carrying_only_the_rows_it_changed();
    a_column_the_player_never_received_gets_no_delta();
    the_column_under_the_player_is_lit_before_the_far_ones();
}
