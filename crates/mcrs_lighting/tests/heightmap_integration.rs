// HEIGHT-02 eager-update + initial-prime integration tests.
//
// Each test builds a fresh Bevy `App` registering `ColumnPlugin` +
// `LightingPlugin`, inserts a stub `BlockLightTable` resource, registers the
// `BlockPlaced` message buffer manually, spawns a dimension and a section,
// runs `FixedUpdate` to let Stage 2.5 prime the heightmap, then either
// asserts the prime result or emits a synthetic `BlockPlaced` and asserts
// the eager-update result.
//
// Test palette state 0 = air-equivalent (emission=0, dampening=0,
// `flags = PROPAGATES_SKYLIGHT_DOWN`). State 1 = solid opaque motion-blocking
// (emission=0, dampening=15,
// `flags = IS_NOT_AIR | IS_SOLID_OPAQUE | IS_MOTION_BLOCKING`).

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::AppExtStates;
use bevy_state::app::StatesPlugin;
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ColumnPlugin, Heightmaps, InChunkColumn};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_lighting::LightingPlugin;
use mcrs_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;
const AIR_STATE: BlockStateId = BlockStateId(0);
const SOLID_STATE: BlockStateId = BlockStateId(1);

fn make_heightmap_test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_block_light_table());
    let dim_entity = spawn_test_dimension(&mut app);
    (app, dim_entity)
}

fn make_stub_block_light_table() -> BlockLightTable {
    let state_count = 2usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] =
        flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn spawn_test_dimension(app: &mut App) -> Entity {
    let entity = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
            dimension_id: DimensionId::new("test:sky"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(entity).insert(HasSkyLight);
    entity
}

fn spawn_test_section(
    app: &mut App,
    dim: Entity,
    chunk_pos: ChunkPos,
    palette: BlockPalette,
) -> Entity {
    app.world_mut()
        .spawn((
            InDimension(dim),
            chunk_pos,
            ChunkEntities::default(),
            Chunk,
            ChunkLoaded,
            palette,
        ))
        .id()
}

fn air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(AIR_STATE);
    p
}

fn solid_floor_palette() -> BlockPalette {
    // Solid blocks at intra-section Y = 0..=3, air above.
    let mut p = BlockPalette::default();
    p.fill(AIR_STATE);
    for y in 0..=3i32 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                p.set(BlockPos::new(x, y, z), SOLID_STATE);
            }
        }
    }
    p
}

fn send_block_placed(app: &mut App, placed: BlockPlaced) {
    app.world_mut()
        .resource_mut::<Messages<BlockPlaced>>()
        .write(placed);
}

fn surface_above_topmost(world: &World, col_entity: Entity, x: usize, z: usize) -> (i32, i32) {
    let h = world
        .get::<Heightmaps>(col_entity)
        .expect("column must have Heightmaps");
    (h.surface_get(x, z), h.motion_blocking_get(x, z))
}

#[test]
fn eager_update_below_surface_early_out() {
    // Solid-floor palette has surface = Y + 1 of topmost solid (intra-section Y=3
    // means absolute Y=3, surface stored as 4). Section is at chunk_pos.y=0 so
    // section base Y = 0. After Stage 2.5 prime: surface = 4, motion_blocking = 4.
    // Placing a non-air block at absolute Y=-30 (well below) must not change either.
    let (mut app, dim) = make_heightmap_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, solid_floor_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let col_entity = app
        .world()
        .get::<InChunkColumn>(section)
        .expect("InChunkColumn back-link missing after prime")
        .0;
    let (surface_before, motion_before) = surface_above_topmost(app.world(), col_entity, 0, 0);
    assert_eq!(surface_before, 4, "primed surface must be 4 above topmost solid");
    assert_eq!(motion_before, 4);

    send_block_placed(
        &mut app,
        BlockPlaced {
            chunk: section,
            chunk_pos,
            block_pos: BlockPos::new(0, -30, 0),
            old_state: AIR_STATE,
            new_state: AIR_STATE,
            flags: BlockUpdateFlags::all(),
        },
    );
    app.world_mut().run_schedule(FixedUpdate);

    let (surface_after, motion_after) = surface_above_topmost(app.world(), col_entity, 0, 0);
    assert_eq!(
        surface_after, surface_before,
        "early-out path must not modify surface"
    );
    assert_eq!(motion_after, motion_before);
}

#[test]
fn eager_update_above_surface_rescan() {
    // Start with the solid-floor palette (surface=4). In production
    // `apply_set_block_request` writes the palette before emitting BlockPlaced,
    // so the test mutates the palette directly then emits the message manually.
    // Placing a solid block at intra-section Y=10 (absolute Y=10) should
    // raise both heightmaps to 11.
    let (mut app, dim) = make_heightmap_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, solid_floor_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let col_entity = app.world().get::<InChunkColumn>(section).unwrap().0;
    let (surface_before, _) = surface_above_topmost(app.world(), col_entity, 0, 0);
    assert_eq!(surface_before, 4);

    app.world_mut()
        .get_mut::<BlockPalette>(section)
        .expect("palette missing")
        .set(BlockPos::new(0, 10, 0), SOLID_STATE);

    send_block_placed(
        &mut app,
        BlockPlaced {
            chunk: section,
            chunk_pos,
            block_pos: BlockPos::new(0, 10, 0),
            old_state: AIR_STATE,
            new_state: SOLID_STATE,
            flags: BlockUpdateFlags::all(),
        },
    );
    app.world_mut().run_schedule(FixedUpdate);

    let (surface_after, motion_after) = surface_above_topmost(app.world(), col_entity, 0, 0);
    assert_eq!(
        surface_after, 11,
        "surface must raise to Y+1 above newly placed solid"
    );
    assert_eq!(motion_after, 11);

    // Cells outside the placed (x, z) must remain unchanged.
    let (other_surface, _) = surface_above_topmost(app.world(), col_entity, 5, 5);
    assert_eq!(other_surface, 4, "unrelated columns must not change");
}

#[test]
fn eager_update_break_above_surface_rescan() {
    // Place a solid at Y=10 (above the solid-floor surface), then break it.
    // The break triggers a rescan that should drop the surface back to 4.
    let (mut app, dim) = make_heightmap_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, solid_floor_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let col_entity = app.world().get::<InChunkColumn>(section).unwrap().0;

    app.world_mut()
        .get_mut::<BlockPalette>(section)
        .unwrap()
        .set(BlockPos::new(0, 10, 0), SOLID_STATE);
    send_block_placed(
        &mut app,
        BlockPlaced {
            chunk: section,
            chunk_pos,
            block_pos: BlockPos::new(0, 10, 0),
            old_state: AIR_STATE,
            new_state: SOLID_STATE,
            flags: BlockUpdateFlags::all(),
        },
    );
    app.world_mut().run_schedule(FixedUpdate);
    assert_eq!(surface_above_topmost(app.world(), col_entity, 0, 0).0, 11);

    app.world_mut()
        .get_mut::<BlockPalette>(section)
        .unwrap()
        .set(BlockPos::new(0, 10, 0), AIR_STATE);
    send_block_placed(
        &mut app,
        BlockPlaced {
            chunk: section,
            chunk_pos,
            block_pos: BlockPos::new(0, 10, 0),
            old_state: SOLID_STATE,
            new_state: AIR_STATE,
            flags: BlockUpdateFlags::all(),
        },
    );
    app.world_mut().run_schedule(FixedUpdate);

    let (surface_after, motion_after) = surface_above_topmost(app.world(), col_entity, 0, 0);
    assert_eq!(
        surface_after, 4,
        "breaking the top solid must drop surface to next-lower solid + 1"
    );
    assert_eq!(motion_after, 4);
}

#[test]
fn initial_prime_on_column_spawn_air_only() {
    let (mut app, dim) = make_heightmap_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, air_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let col_entity = app.world().get::<InChunkColumn>(section).unwrap().0;
    for &(x, z) in &[(0, 0), (5, 7), (15, 15)] {
        let (surface, motion) = surface_above_topmost(app.world(), col_entity, x, z);
        assert_eq!(
            surface, TEST_DIM_MIN_Y,
            "air-only column surface must equal dimension min_y at ({x}, {z})"
        );
        assert_eq!(
            motion, TEST_DIM_MIN_Y,
            "air-only column motion_blocking must equal dimension min_y at ({x}, {z})"
        );
    }
}

#[test]
fn initial_prime_on_column_spawn_with_solid_floor() {
    let (mut app, dim) = make_heightmap_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, solid_floor_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let col_entity = app.world().get::<InChunkColumn>(section).unwrap().0;
    for &(x, z) in &[(0, 0), (8, 12), (15, 0)] {
        let (surface, motion) = surface_above_topmost(app.world(), col_entity, x, z);
        assert_eq!(
            surface, 4,
            "solid floor up to Y=3 must yield surface = 4 (Y + 1 above topmost) at ({x}, {z})"
        );
        assert_eq!(
            motion, 4,
            "motion_blocking must match surface for purely solid floor at ({x}, {z})"
        );
    }
}
