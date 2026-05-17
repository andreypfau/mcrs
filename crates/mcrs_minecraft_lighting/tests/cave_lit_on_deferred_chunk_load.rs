// Regression test for the cave-fully-lit-on-load bug.
//
// Root cause: when a column's chunks arrive across two separate FixedUpdate
// ticks, the heightmap was primed from incomplete data (surface at min_y sentinel
// for missing chunks). `seed_sky_initial` read the stale heightmap and
// classified cave-air chunks as Case A ("fully above surface") → Uniform(15).
//
// Fix: `prime_heightmaps_on_column_spawn` gates on column completeness. It
// returns early if any chunk slot is still None, so the prime never runs on a
// partial column. `seed_block_emitters` and `seed_sky_initial` are only
// triggered (via `BlockNeedsInitialSeed` / `SkyNeedsInitialSeed`) after the
// prime fires on the complete column, guaranteeing the heightmap is correct on
// the first and only read.
//
// Geometry:
//   Column XZ = (0, 0). Dimension min_y = 0, height = 32.
//   Two chunks: chunk_y=0 (world y=0..15) all air (cave space),
//                 chunk_y=1 (world y=16..31) all stone (cave roof).
//   After prime: surface_get(x, z) = 32 for all (x, z).
//   Cave chunk: chunk_base_y=0, chunk_top_y=15; s=32 > 15 → Case C → dark.
//   Stone chunk: Case C likewise.
//
// Test procedure:
//   Tick 1: spawn only the cave-air chunk. `prime_heightmaps_on_column_spawn`
//           fires on Changed<ColumnChunks> but the column is incomplete (stone
//           slot still None) → early return. The seed systems do NOT fire (the
//           per-channel `BlockNeedsInitialSeed` / `SkyNeedsInitialSeed`
//           markers were not inserted).
//   Tick 2: spawn the surface chunk. Changed<ColumnChunks> fires again; column
//           is now complete → prime runs with full data → surface=32 for all
//           columns → inserts the per-channel markers on both chunks.
//           `seed_block_emitters` and `seed_sky_initial` fire for both chunks
//           with the correct heightmap. Cave chunk is Case C (all below
//           surface) → sky_light=0.
//   Ticks 3-6: extra convergence ticks.
//
// After convergence, sky_light at any cell of the cave-air chunk must be 0.
// Under the pre-fix behaviour it would be 15 (stale heightmap case).

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::ColumnPlugin;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_lighting::components::SkyLight;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);

// Two-chunk dimension so the completeness gate triggers after exactly two
// chunk arrivals, reproducing the deferred-load scenario without needing to
// spawn the full 24-chunk overworld column.
const TEST_DIM_MIN_Y: i32 = 0;
const TEST_DIM_HEIGHT: u32 = 32;

fn make_test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_table());
    let dim = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
            dimension_id: DimensionId::new("test:sky"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(dim).insert(HasSkyLight);
    (app, dim)
}

fn make_stub_table() -> BlockLightTable {
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
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn spawn_chunk(app: &mut App, dim: Entity, chunk_pos: ChunkPos, palette: BlockPalette) -> Entity {
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

fn all_air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(AIR);
    p
}

fn all_stone_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(STONE);
    p
}

fn sky_level_at(app: &App, chunk: Entity, lx: usize, ly: usize, lz: usize) -> u8 {
    app.world()
        .get::<SkyLight>(chunk)
        .expect("SkyLight missing")
        .0
        .get(lx, ly, lz)
}

/// When a column's cave-air chunk (no non-air blocks) loads one tick BEFORE
/// its stone surface chunk, the completeness gate defers the heightmap prime
/// and per-channel needs-initial marker insertion until the surface chunk
/// arrives. `seed_sky_initial` then runs once with the fully-primed heightmap
/// and classifies the cave chunk as Case C (all below surface) → sky_light=0.
#[test]
fn cave_air_chunk_dark_when_surface_chunk_arrives_later() {
    let (mut app, dim) = make_test_app();

    // chunk_pos.y=0 → world y=0..15, all air (enclosed cave space).
    let cave_chunk_pos = ChunkPos::new(0, 0, 0);
    // chunk_pos.y=1 → world y=16..31, all stone (the cave roof).
    let surface_chunk_pos = ChunkPos::new(0, 1, 0);

    // Tick 1: spawn only the cave-air chunk. The column is born here.
    // `prime_heightmaps_on_column_spawn` fires on Changed<ColumnChunks> but
    // returns early (stone slot still None). The per-channel needs-initial
    // markers are NOT inserted. The seed systems have nothing to process.
    let cave_chunk = spawn_chunk(&mut app, dim, cave_chunk_pos, all_air_palette());
    app.world_mut().run_schedule(FixedUpdate);

    // Tick 2: surface chunk arrives (simulating the deferred-load race).
    // Changed<ColumnChunks> fires; column is now complete → prime runs with
    // full data → inserts the per-channel needs-initial markers on both
    // chunks. `seed_block_emitters` + `seed_sky_initial` fire for both with
    // correct heightmap (surface=32). Cave chunk: chunk_base_y=0, s=32 >
    // chunk_top_y=15 → Case C → dark.
    let _surface_chunk = spawn_chunk(&mut app, dim, surface_chunk_pos, all_stone_palette());
    app.world_mut().run_schedule(FixedUpdate);

    // Extra convergence ticks.
    for _ in 0..4 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    // Every cell in the cave-air chunk must be sky_light=0. The cave is fully
    // enclosed above by stone; sky light cannot legitimately reach it.
    for z in 0..16usize {
        for x in 0..16usize {
            for y in 0..16usize {
                let level = sky_level_at(&app, cave_chunk, x, y, z);
                assert_eq!(
                    level, 0,
                    "cave-air cell ({x}, y_local={y}, {z}) must have sky_light=0 \
                     when enclosed above by stone and surface chunk loads late; got {level}"
                );
            }
        }
    }
}
