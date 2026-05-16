// INT-04: cold-boot warm-up via `Added<ChunkLoaded>` in `AppState::Playing`.
//
// Builds the minimum app surface that exercises the full warm-up chain:
// `attach_lighting_state` (`AttachState` set) inserts the per-chunk storage
// components and `ChunkNeedsInitialLight`; `seed_initial_light` consumes the
// marker and seeds emitter / sky sources; `pull_neighbor_edge_levels` (driven
// by `Added<ChunkLoaded>`) merges the neighbour edge cells; the bounded
// `light_converge_driver` runs the BFS-style convergence under
// `LightConvergeSchedule`; the `EmitDirty` stage clears the safety-net
// `LightDirty` markers once queues drain.
//
// The test asserts (a) the storage components are populated, (b) no
// `LightDirty` marker remains on any chunk after warm-up, and (c) none of
// the bounded-loop / pending-egress / cross-dim telemetry counters
// incremented across the run.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ColumnPlugin, InColumn};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::components::{BlockLight, LightDirty, SkyLight};
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::LightingPlugin;
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

fn air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(BlockStateId(0));
    p
}

#[test]
fn playing_tick_warmup_propagates_initial_light_without_residual_dirty() {
    let _lock = TELEMETRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let (mut app, dim) = make_test_app(true);

    // Drive AppState::Playing for the cold-boot scenario. Production-side
    // `WorldPlugin` emits `ChunkLoaded` only in Playing; tests bypass the
    // state-machine ramp by setting `NextState` directly.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);

    let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), air_palette());

    // Three ticks: tick 1 reconciles the column + chunk index and attaches
    // lighting state; tick 2 runs `seed_initial_light` +
    // `pull_neighbor_edge_levels` + convergence + emit-dirty; tick 3 lets
    // the safety-net `clear_light_dirty_safety_net` drain any residual
    // marker observed after queues are empty.
    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    {
        let world = app.world();
        assert!(
            world.get::<BlockLight>(chunk).is_some(),
            "BlockLight component must be populated after warm-up"
        );
        assert!(
            world.get::<SkyLight>(chunk).is_some(),
            "SkyLight component must be populated after warm-up (sky-having dim)"
        );
        let in_col = world
            .get::<InColumn>(chunk)
            .expect("chunk must have InColumn back-link after lifecycle reconcile");
        assert_ne!(in_col.0, Entity::PLACEHOLDER);
    }

    let mut dirty_q = app
        .world_mut()
        .query_filtered::<Entity, With<LightDirty>>();
    let dirty_count = dirty_q.iter(app.world()).count();
    assert_eq!(
        dirty_count, 0,
        "no residual LightDirty marker must remain after 3 warm-up ticks"
    );

    let after = snapshot();
    assert_eq!(
        after.capped, before.capped,
        "warm-up must not hit MAX_ITERATIONS or HARD_BUDGET (LIGHT_CONVERGE_CAPPED_TOTAL must not increment)"
    );
    assert_eq!(
        after.overflow, before.overflow,
        "warm-up must not hit PENDING_EGRESS_CAP (LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL must not increment)"
    );
    assert_eq!(
        after.cross_dim, before.cross_dim,
        "warm-up must not fire cross-dim guard (LIGHT_CROSS_DIM_VIOLATIONS_TOTAL must not increment)"
    );
}
