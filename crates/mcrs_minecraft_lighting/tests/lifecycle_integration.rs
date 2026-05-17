// ECS smoke tests for the chunk-column lighting lifecycle.
//
// Each test builds a minimal Bevy `App` registering `ColumnPlugin` +
// `LightingPlugin`, inserts a stub `BlockLightTable` resource keyed by
// `BlockStateId`, spawns dimensions and chunks directly, runs a single
// `FixedUpdate` tick, and asserts on the resulting component graph.
//
// The stub table replaces the production `build_block_light_table` path:
// production runs the latter on `OnEnter(AppState::WorldgenFreeze)`, but the
// tests bypass the state machine for speed. Test palette state 0 stands in
// for "air-equivalent" (emission=0, dampening=0, IS_NOT_AIR=0,
// PROPAGATES_SKYLIGHT_DOWN=1). State 1 stands in for a solid opaque
// motion-blocking block.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    Column, ColumnPos, ColumnIndex, ColumnPlugin, InColumn, ColumnChunks,
    ChunkLookup,
};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_lighting::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, BlockNeedsInitialSeed, IsAllAir,
    SkyEgress, SkyIncoming, SkyLight, SkyLightWorkspace, SkyNeedsInitialSeed,
};
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;

fn make_test_app(sky: bool) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_block_light_table());
    let dim_entity = spawn_test_dimension(&mut app, sky);
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

fn spawn_test_dimension(app: &mut App, sky: bool) -> Entity {
    let entity = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
            dimension_id: DimensionId::new(if sky {
                "test:sky"
            } else {
                "test:skyless"
            }),
            ..Default::default()
        })
        .id();
    if sky {
        app.world_mut().entity_mut(entity).insert(HasSkyLight);
    }
    entity
}

fn spawn_test_chunk(
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

fn solid_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(BlockStateId(1));
    p
}

fn air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(BlockStateId(0));
    p
}

#[test]
fn single_chunk_in_sky_dim_attaches_all_components() {
    let (mut app, dim) = make_test_app(true);
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let chunk = spawn_test_chunk(&mut app, dim, chunk_pos, air_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<Column>>();
    let column_count = q.iter(app.world()).count();
    let world = app.world();
    assert_eq!(column_count, 1, "exactly one Column entity expected");

    let in_col = world
        .get::<InColumn>(chunk)
        .expect("chunk must have InColumn back-link");
    let col_entity = in_col.0;

    assert!(world.get::<BlockLight>(chunk).is_some(), "BlockLight missing");
    assert!(world.get::<BlockEgress>(chunk).is_some(), "BlockEgress missing");
    assert!(
        world.get::<BlockIncoming>(chunk).is_some(),
        "BlockIncoming missing"
    );
    assert!(
        world.get::<BlockLightWorkspace>(chunk).is_some(),
        "BlockLightWorkspace missing"
    );
    assert!(world.get::<SkyLight>(chunk).is_some(), "SkyLight missing");
    assert!(world.get::<SkyEgress>(chunk).is_some(), "SkyEgress missing");
    assert!(
        world.get::<SkyIncoming>(chunk).is_some(),
        "SkyIncoming missing"
    );
    assert!(
        world.get::<SkyLightWorkspace>(chunk).is_some(),
        "SkyLightWorkspace missing"
    );
    // The per-channel needs-initial markers are inserted by
    // `prime_heightmaps_on_column_spawn` once the heightmap scan finalises,
    // and consumed by `seed_block_emitters` / `seed_sky_initial` within the
    // same FixedUpdate tick under the CROSS-08 chain. After one tick both
    // markers must be gone (they would only remain if the seed systems
    // failed to run, which would break single-tick convergence).
    assert!(
        world.get::<BlockNeedsInitialSeed>(chunk).is_none(),
        "BlockNeedsInitialSeed must be consumed by seed_block_emitters within the tick"
    );
    assert!(
        world.get::<SkyNeedsInitialSeed>(chunk).is_none(),
        "SkyNeedsInitialSeed must be consumed by seed_sky_initial within the tick"
    );
    assert!(
        world.get::<IsAllAir>(chunk).is_some(),
        "IsAllAir must be set for all-air palette"
    );

    let column_index = world
        .get::<ColumnIndex>(dim)
        .expect("dimension must have ColumnIndex");
    let slot = column_index
        .0
        .get(&ColumnPos::from(chunk_pos))
        .expect("ColumnSlot must exist for the spawned chunk's column");
    assert_eq!(slot.section_count, 1);
    assert_eq!(slot.entity, col_entity);

    let chunk_index = world
        .get::<ColumnChunks>(col_entity)
        .expect("column entity must have ColumnChunks");
    assert_eq!(
        chunk_index.lookup(chunk_pos.y),
        ChunkLookup::Loaded(chunk)
    );
}

#[test]
fn multi_chunk_in_same_column_share_column() {
    let (mut app, dim) = make_test_app(true);
    let s_low = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), air_palette());
    let s_high = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 5, 0), air_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<Column>>();
    let column_count = q.iter(app.world()).count();
    let world = app.world();
    assert_eq!(column_count, 1, "two chunks at same XZ share one column");

    let col_low = world.get::<InColumn>(s_low).unwrap().0;
    let col_high = world.get::<InColumn>(s_high).unwrap().0;
    assert_eq!(col_low, col_high);

    let column_index = world.get::<ColumnIndex>(dim).unwrap();
    let slot = column_index
        .0
        .get(&ColumnPos::new(0, 0))
        .expect("column slot missing");
    assert_eq!(slot.section_count, 2);
}

#[test]
fn unload_one_chunk_keeps_column_alive() {
    use mcrs_engine::world::chunk::ChunkUnloading;

    let (mut app, dim) = make_test_app(true);
    let s_low = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), air_palette());
    let _s_high = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 5, 0), air_palette());

    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().entity_mut(s_low).insert(ChunkUnloading);
    app.world_mut().run_schedule(FixedUpdate);

    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<Column>>();
    let column_count = q.iter(app.world()).count();
    let world = app.world();
    assert_eq!(column_count, 1, "column entity must outlive partial unload");

    let column_index = world.get::<ColumnIndex>(dim).unwrap();
    let slot = column_index
        .0
        .get(&ColumnPos::new(0, 0))
        .expect("column slot must still exist");
    assert_eq!(slot.section_count, 1);
}

#[test]
fn unload_last_chunk_despawns_column() {
    use mcrs_engine::world::chunk::ChunkUnloading;

    let (mut app, dim) = make_test_app(true);
    let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), air_palette());

    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut().entity_mut(chunk).insert(ChunkUnloading);
    app.world_mut().run_schedule(FixedUpdate);

    let mut q = app
        .world_mut()
        .query_filtered::<Entity, With<Column>>();
    let column_count = q.iter(app.world()).count();
    let world = app.world();
    assert_eq!(
        column_count, 0,
        "column entity must despawn when its last chunk unloads"
    );

    let column_index = world.get::<ColumnIndex>(dim).unwrap();
    assert!(
        !column_index.0.contains_key(&ColumnPos::new(0, 0)),
        "ColumnIndex must drop the entry for the despawned column"
    );
}

#[test]
fn single_chunk_in_skyless_dim_has_no_sky_components() {
    let (mut app, dim) = make_test_app(false);
    let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), air_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let world = app.world();
    assert!(world.get::<BlockLight>(chunk).is_some());
    assert!(world.get::<BlockEgress>(chunk).is_some());
    assert!(world.get::<BlockIncoming>(chunk).is_some());
    assert!(world.get::<BlockLightWorkspace>(chunk).is_some());
    assert!(
        world.get::<SkyLight>(chunk).is_none(),
        "SkyLight must not exist in a skyless dimension"
    );
    assert!(world.get::<SkyEgress>(chunk).is_none(), "SkyEgress leaked");
    assert!(
        world.get::<SkyIncoming>(chunk).is_none(),
        "SkyIncoming leaked"
    );
    assert!(
        world.get::<SkyLightWorkspace>(chunk).is_none(),
        "SkyLightWorkspace leaked"
    );
    // `BlockNeedsInitialSeed` is inserted by `prime_heightmaps_on_column_spawn`
    // and consumed by `seed_block_emitters` within the same tick (skyless dims
    // still receive the block-channel marker for their emitter-scan path).
    // `SkyNeedsInitialSeed` is never inserted in a skyless dim. After one tick
    // both must be absent.
    assert!(
        world.get::<BlockNeedsInitialSeed>(chunk).is_none(),
        "BlockNeedsInitialSeed must be consumed by seed_block_emitters within the tick"
    );
    assert!(
        world.get::<SkyNeedsInitialSeed>(chunk).is_none(),
        "SkyNeedsInitialSeed must never appear on a skyless-dim chunk"
    );
}

#[test]
fn mixed_palette_chunk_has_no_is_all_air() {
    let (mut app, dim) = make_test_app(true);
    let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), solid_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let world = app.world();
    assert!(
        world.get::<IsAllAir>(chunk).is_none(),
        "Solid palette must not be marked IsAllAir"
    );
}

#[test]
fn cross_dim_partitioning_smoke() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_block_light_table());

    let dim_a = spawn_test_dimension(&mut app, true);
    let dim_b = spawn_test_dimension(&mut app, false);

    let sec_a = spawn_test_chunk(&mut app, dim_a, ChunkPos::new(0, 0, 0), air_palette());
    let sec_b = spawn_test_chunk(&mut app, dim_b, ChunkPos::new(0, 0, 0), air_palette());

    app.world_mut().run_schedule(FixedUpdate);

    let world = app.world();
    let col_a = world.get::<InColumn>(sec_a).unwrap().0;
    let col_b = world.get::<InColumn>(sec_b).unwrap().0;
    assert_ne!(col_a, col_b, "columns in different dimensions must differ");

    let col_a_dim = world.get::<InDimension>(col_a).unwrap().0;
    let col_b_dim = world.get::<InDimension>(col_b).unwrap().0;
    assert_eq!(col_a_dim, dim_a);
    assert_eq!(col_b_dim, dim_b);
}
