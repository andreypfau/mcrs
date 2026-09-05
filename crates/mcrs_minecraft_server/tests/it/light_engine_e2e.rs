use bevy_app::{App, AppLabel, Last};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Entity, IntoScheduleConfigs, ResMut, Resource};
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_light::prelude::{
    BlockLight, LightBudget, LightEpoch, LightWorkQueue, PendingEdits, SkyLight,
};
use mcrs_minecraft_protocol::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_server::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use mcrs_minecraft_server::world::entity::player::column_view::ColumnView;
use mcrs_minecraft_server::world::light::emit_light_updates;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};
use mcrs_minecraft_world::block::definition::Blocks;
use mcrs_voxel_math::{BlockPos, ChunkPos, ColumnPos};
use mcrs_voxel_storage::VoxelId;
use mcrs_voxel_world::aoi::PlayerObservers;
use mcrs_voxel_world::entity::physics::Transform;
use mcrs_voxel_world::entity::player::Player;
use mcrs_voxel_world::world::dimension::InDimension;
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoaded;
use mcrs_voxel_world::world::storage::column::ColumnIndex;
use mcrs_voxel_world::world::sub_app::DimAppLabel;

use crate::host_app;

const SECTIONS: std::ops::Range<i32> = -4..20;
const STONE_SECTION_Y: i32 = 0;

fn state(blocks: &Blocks, name: &str) -> VoxelId {
    blocks
        .block(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
        .default_state_id
        .into()
}

fn filled(block: VoxelId) -> BlockPalette {
    let mut palette = BlockPalette::default();
    palette.fill(block);
    palette
}

/// Spawn one column of section entities exactly as `process_completed_columns`
/// does, then pump until the engine has nothing left to do.
fn light_one_column(app: &mut App, label: DimAppLabel, stone_floor: bool) {
    let blocks = app.world().resource::<Blocks>().clone();
    let air = state(&blocks, "minecraft:air");
    let stone = state(&blocks, "minecraft:stone");

    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists");
    let world = sub_app.world_mut();
    let dimension = world
        .query_filtered::<Entity, bevy_ecs::prelude::With<ColumnIndex>>()
        .iter(world)
        .next()
        .expect("the dimension entity");
    for y in SECTIONS {
        let solid = stone_floor && y == STONE_SECTION_Y;
        world.spawn((
            ChunkPos::new(0, y, 0),
            InDimension(dimension),
            filled(if solid { stone } else { air }),
            ChunkLoaded,
        ));
    }
    settle(app, label);
}

fn settle(app: &mut App, label: DimAppLabel) {
    for _ in 0..400 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let world = app
            .sub_apps()
            .sub_apps
            .get(&label.intern())
            .expect("the dimension sub-app exists")
            .world();
        if !world.resource::<LightEpoch>().is_running()
            && world.resource::<PendingEdits>().is_empty()
            && world.resource::<LightWorkQueue>().0.is_empty()
        {
            return;
        }
    }
    panic!("light never settled");
}

fn published(app: &mut App, label: DimAppLabel, pos: ChunkPos) -> (BlockLight, SkyLight) {
    let world = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists")
        .world_mut();
    let mut sections = world.query::<(&ChunkPos, &BlockLight, &SkyLight)>();
    let (_, block, sky) = sections
        .iter(world)
        .find(|(at, _, _)| **at == pos)
        .unwrap_or_else(|| panic!("{pos:?} was published"));
    (block.clone(), sky.clone())
}

fn spawn_dimension(id: &str, sky: bool) -> (App, DimAppLabel) {
    let mut app = host_app::make_host_app();
    host_app::enable_lighting(&mut app);
    host_app::enqueue_spawn(&mut app, id, sky);
    drain_dim_spawn_queue(&mut app);

    let mut handles = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handle = handles
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("one sub-app handle");
    (app, DimAppLabel(handle))
}

#[test]
fn sky_light_falls_where_the_blocks_say_it_should() {
    let (mut app, label) = spawn_dimension("test:overworld", true);
    light_one_column(&mut app, label, true);

    let (_, sky) = published(&mut app, label, ChunkPos::new(0, 19, 0));
    assert_eq!(sky.0.get(8, 15, 8), 15, "the top of the world sees the sky");

    let (_, sky) = published(&mut app, label, ChunkPos::new(0, STONE_SECTION_Y + 1, 0));
    assert_eq!(
        sky.0.get(8, 0, 8),
        15,
        "the cell above the floor is a source"
    );

    let (_, sky) = published(&mut app, label, ChunkPos::new(0, STONE_SECTION_Y - 1, 0));
    assert_eq!(sky.0.get(8, 15, 8), 0, "the floor casts a shadow under it");
}

/// Stands in for `apply_voxel_set_requests`, which writes the palette and then
/// the message; the chunk index it resolves through is not wired here.
fn place_torch(app: &mut App, label: DimAppLabel, at: BlockPos) -> u8 {
    let blocks = app.world().resource::<Blocks>().clone();
    let torch = state(&blocks, "minecraft:torch");
    let emission = blocks.state(torch.into()).light_emission;

    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists");
    let chunk_pos = ChunkPos::from(at);
    let mut sections = sub_app.world_mut().query::<(Entity, &ChunkPos)>();
    let chunk = sections
        .iter(sub_app.world())
        .find(|(_, pos)| **pos == chunk_pos)
        .map(|(entity, _)| entity)
        .expect("the section holding the torch");
    sub_app
        .world_mut()
        .get_mut::<BlockPalette>(chunk)
        .expect("the section has blocks")
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

#[test]
fn a_placed_torch_lights_its_neighbourhood() {
    let (mut app, label) = spawn_dimension("test:overworld", true);
    light_one_column(&mut app, label, true);

    let at = BlockPos::new(8, -8, 8);
    let emission = place_torch(&mut app, label, at);
    assert!(emission > 1, "a torch emits light");

    let chunk_pos = ChunkPos::from(at);
    let (block, _) = published(&mut app, label, chunk_pos);
    assert_eq!(block.0.get(8, 8, 8), emission);
    assert_eq!(block.0.get(9, 8, 8), emission - 1);
    assert_eq!(block.0.get(8, 8, 10), emission - 2);
}

#[test]
fn a_dimension_without_a_sky_publishes_none() {
    let (mut app, label) = spawn_dimension("test:nether", false);
    light_one_column(&mut app, label, false);

    for y in [19, 0, -4] {
        let (_, sky) = published(&mut app, label, ChunkPos::new(0, y, 0));
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

/// Reads the dim's outbox in the same schedule the emitter writes it, before
/// `flush_from_dim_outbox` drains it in the next tick's `FixedLast`.
fn capture_light_updates(
    mut reader: MessageReader<OutboundPlayerPacket>,
    mut captured: ResMut<CapturedLightUpdates>,
) {
    for packet in reader.read() {
        if matches!(packet.data, PacketPayload::LightUpdate { .. }) {
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

fn nibble(chunk: &mcrs_minecraft_protocol::chunk::LightChunk, x: usize, y: usize, z: usize) -> u8 {
    let idx = (y << 8) | (z << 4) | x;
    (chunk.0[idx >> 1] >> ((idx & 1) * 4)) & 0x0F
}

/// Light one column, give it a viewing player that has (or has not) already
/// received it, then place a torch and collect what the emitter sent.
fn torch_delta(already_sent: bool) -> (Vec<OutboundPlayerPacket>, Entity) {
    let (mut app, label) = spawn_dimension("test:overworld", true);
    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists");
    sub_app.init_resource::<CapturedLightUpdates>();
    sub_app.add_systems(Last, capture_light_updates.after(emit_light_updates));

    light_one_column(&mut app, label, true);

    let column = ColumnPos::new(0, 0);
    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists");
    let world = sub_app.world_mut();
    let mut view = ColumnView::default();
    if already_sent {
        view.sent_columns.insert(column);
    }
    let player = world.spawn((Player, view)).id();
    let column_entity = world
        .query::<&ColumnIndex>()
        .iter(world)
        .next()
        .and_then(|index| index.0.get(&column).map(|slot| slot.entity))
        .expect("the column the sections reconciled into");
    let mut observers = PlayerObservers::default();
    observers.0.push(player);
    world.entity_mut(column_entity).insert(observers);
    world.resource_mut::<CapturedLightUpdates>().0.clear();

    place_torch(&mut app, label, torch_at());

    let captured = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists")
        .world()
        .resource::<CapturedLightUpdates>()
        .0
        .clone();
    (captured, player)
}

#[test]
fn a_torch_sends_one_delta_carrying_only_the_rows_it_changed() {
    let (captured, player) = torch_delta(true);

    assert_eq!(captured.len(), 1, "one packet for one column");
    let packet = &captured[0];
    match &packet.target {
        PacketTarget::PlayerSet(set) => assert_eq!(set.as_slice(), [player]),
        other => panic!("expected a player set, got {other:?}"),
    }
    assert_eq!(
        packet.priority,
        PacketPriority::High,
        "a column's light is sent in full once, so a shed delta is never made good"
    );
    let PacketPayload::LightUpdate { column, light_data } = &packet.data else {
        unreachable!("filtered above");
    };
    assert_eq!(*column, ColumnPos::new(0, 0));

    let rows = unpack_light_data(light_data, WIRE_ROWS).expect("the delta decodes");
    let torch_row = wire_row(ChunkPos::from(torch_at()).y);
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

#[test]
fn both_sections_one_torch_reaches_ride_in_the_same_packet() {
    let (captured, _) = torch_delta(true);

    assert_eq!(captured.len(), 1, "one packet, not one per section");
    let PacketPayload::LightUpdate { light_data, .. } = &captured[0].data else {
        unreachable!("filtered above");
    };
    let rows = unpack_light_data(light_data, WIRE_ROWS).expect("the delta decodes");
    for section_y in [-1, -2] {
        assert!(
            matches!(rows.block[wire_row(section_y)], RowLight::Filled(_)),
            "section {section_y} is lit by a torch 14 blocks tall"
        );
    }
}

#[test]
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

#[test]
fn the_column_under_the_player_is_lit_before_the_far_ones() {
    let (mut app, label) = spawn_dimension("test:overworld", true);
    let blocks = app.world().resource::<Blocks>().clone();
    let stone = state(&blocks, "minecraft:stone");
    let air = state(&blocks, "minecraft:air");

    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&label.intern())
        .expect("the dimension sub-app exists");
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
            world.spawn((
                ChunkPos::new(x, y, z),
                InDimension(dimension),
                filled(if solid { stone } else { air }),
                ChunkLoaded,
            ));
        }
    }

    let mut lit: Vec<(ColumnPos, usize)> = Vec::new();
    for tick in 0..400 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let world = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&label.intern())
            .expect("the dimension sub-app exists")
            .world_mut();
        let newly: Vec<ColumnPos> = world
            .query_filtered::<&ChunkPos, bevy_ecs::prelude::With<BlockLight>>()
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
