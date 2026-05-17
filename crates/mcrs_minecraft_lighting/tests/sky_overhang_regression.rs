// Regression tests for three sky-light golden coordinates where sky light
// was incorrectly zero in naturally-generated overhangs and cave pockets.
//
// All three tests use a synthetic world geometry that reproduces the
// lighting-relevant terrain structure without running the full worldgen
// pipeline. They verify that sky_light > 0 at each reported air-block
// coordinate after a complete FixedUpdate convergence run.
//
// Coordinate summary (world space → chunk-local for chunk_y=4):
//   Scenario A: intra-chunk overhang.  Air at (lx=8, local_y=14, lz=4)
//               of chunk (0,4,0).  Open-sky column at lx=7 has surface at
//               local_y=5; cells above are at 15.  The air pocket at lx=8
//               is enclosed except toward lx=7 at y=14.
//   Scenario B: cross-chunk, Uniform(15) neighbour.  Air at (lx=8,
//               local_y=4, lz=4) of chunk (1,4,0).  Chunk (0,4,0) is all
//               open sky → Uniform(15).  Both chunks load in the same
//               tick.
//   Scenario C: same cross-chunk scenario as scenario B but the dark
//               column at (1,4,0) has TWO separate air pockets at
//               local_y=4 AND local_y=13, separated by solid rock at
//               local_y=5..12.  Both must read > 0.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ColumnPlugin};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_lighting::components::SkyLight;
use mcrs_minecraft_lighting::table::{flag_bits, BlockStateLightTable};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);

const TEST_DIM_HEIGHT: u32 = 16;
const TEST_DIM_MIN_Y: i32 = 64;

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

fn make_stub_table() -> BlockStateLightTable {
    const SIZE: usize = 2;
    let mut emission = vec![0u8; SIZE].into_boxed_slice();
    let mut dampening = vec![0u8; SIZE].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); SIZE].into_boxed_slice();
    let mut flags = vec![0u8; SIZE].into_boxed_slice();
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn spawn_chunk_with_palette(app: &mut App, dim: Entity, chunk_pos: ChunkPos, palette: BlockPalette) -> Entity {
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

fn sky_level_at(app: &App, chunk: Entity, lx: usize, local_y: usize, lz: usize) -> u8 {
    app.world()
        .get::<SkyLight>(chunk)
        .expect("SkyLight missing")
        .0
        .get(lx, local_y, lz)
}

/// Builds an all-air palette for the given chunk.
fn all_air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(AIR);
    p
}

// ── Intra-chunk overhang regression ────────────────────────────────────
//
// Geometry (single chunk, chunk_pos=(0,4,0), chunk_y=4, world y=64..79):
//
//   Column lx=7, lz=4: solid y=64..67 (local_y=0..3), air y=68..79
//   (local_y=4..15).  Heightmap surface = world_y 67+1 = 68 → local_y=4.
//   Seed placed at local_y=4 by the normal Case-B path.  Storage has level=15
//   at local_y=4..15 for this column.
//
//   Column lx=8, lz=4: solid everywhere except air at local_y=14 (the
//   cavity).  Top two cells (local_y=14..15) are actually: local_y=15=solid,
//   local_y=14=air, local_y=13..0=solid.
//   Heightmap surface = world_y 79+1 = 80 (first non-air from top =
//   local_y=15 = world y=79) → s=80 > chunk_top_y=79.
//   All 16 cells zeroed by Case-B seeding.  Under the bug, no BFS entry
//   exists for column lx=7 at local_y=14, so (lx=8, local_y=14, lz=4)
//   remains at 0.  The fix seeds all y ≥ surface_local for Case-B columns.
//
// Expected: sky_light at (lx=8, local_y=14, lz=4) > 0 after convergence.
#[test]
fn intra_chunk_overhang_air_pocket_gets_lit() {
    let (mut app, dim) = make_test_app();

    let chunk_pos = ChunkPos::new(0, 4, 0);

    let mut palette = all_air_palette();
    let base_x = chunk_pos.x * 16;
    let base_y = chunk_pos.y * 16;
    let base_z = chunk_pos.z * 16;

    // Column (lx=8, lz=4): solid at all local_y except air at local_y=14.
    for local_y in 0..16i32 {
        if local_y != 14 {
            palette.set(
                BlockPos::new(base_x + 8, base_y + local_y, base_z + 4),
                STONE,
            );
        }
    }

    // Column (lx=7, lz=4): solid at local_y=0..3, air at local_y=4..15.
    // (leave local_y=4..15 as air, only fill local_y=0..3 with stone)
    for local_y in 0..4i32 {
        palette.set(
            BlockPos::new(base_x + 7, base_y + local_y, base_z + 4),
            STONE,
        );
    }

    let chunk = spawn_chunk_with_palette(&mut app, dim, chunk_pos, palette);

    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level = sky_level_at(&app, chunk, 8, 14, 4);
    assert!(
        level > 0,
        "overhang regression: sky_light at (lx=8, local_y=14, lz=4) must be > 0 after \
         horizontal propagation from adjacent open-sky column; got {level}"
    );
}

// ── Cross-chunk Uniform(15) neighbour regression ──────────────────────────
//
// Geometry (two chunks, chunk A at (0,4,0) and chunk B at (1,4,0)):
//
//   Chunk A: all columns fully open sky (all air) → chunk_y=4 becomes
//   Uniform(15).
//
//   Chunk B: column (lx=8, lz=4) has solid at local_y=0..3 and local_y=5..15,
//   with air at local_y=4 only.  All other columns are all-solid (simulating a
//   mountain wall with a single horizontal tunnel at y=68).  The Heightmap
//   surface for column (lx=8, lz=4) in chunk B is far above chunk_top_y
//   (the top of the column visible from the sky is the solid at local_y=5 and
//   above, but since the whole column above the air pocket is solid, the scanner
//   finds solid at local_y=5 and records s = world_y(5)+1 = 70).
//   Wait — actually: column (8,4) in chunk B has solid at local_y=5..15 and
//   air at local_y=4, solid at local_y=0..3.  Scanning from top: local_y=15
//   solid → s = 64+15+1 = 80 > chunk_top_y=79.  All cells zeroed.
//
//   Both chunks Added<ChunkLoaded> in same tick.  The pull-from-Uniform(15)
//   path must fire even when both neighbours are newly-loaded.
//
// Expected: sky_light at (lx=0 in chunk B frame = lx=0, local_y=4, lz=4) > 0.
// (The pull injects from chunk A's East face into chunk B's West face at all y;
// the BFS inside chunk B then propagates +X from lx=0 to lx=8 at local_y=4
// if that path is open.  For this test, make the path open: lx=0..8 at
// local_y=4, lz=4 are all air.)
#[test]
fn cross_chunk_uniform_neighbour_lights_lower_air_pocket() {
    let (mut app, dim) = make_test_app();

    let chunk_a_pos = ChunkPos::new(0, 4, 0);
    let chunk_b_pos = ChunkPos::new(1, 4, 0);

    let palette_a = all_air_palette();

    // Chunk B: all solid except a horizontal tunnel at local_y=4, lz=4,
    // lx=0..15.  The tunnel provides the air path from the West face to lx=8.
    let base_bx = chunk_b_pos.x * 16;
    let base_by = chunk_b_pos.y * 16;
    let base_bz = chunk_b_pos.z * 16;
    let mut palette_b = BlockPalette::default();
    palette_b.fill(STONE);
    for lx in 0..16i32 {
        palette_b.set(BlockPos::new(base_bx + lx, base_by + 4, base_bz + 4), AIR);
    }

    let _chunk_a = spawn_chunk_with_palette(&mut app, dim, chunk_a_pos, palette_a);
    let chunk_b = spawn_chunk_with_palette(&mut app, dim, chunk_b_pos, palette_b);

    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level = sky_level_at(&app, chunk_b, 8, 4, 4);
    assert!(
        level > 0,
        "cross-chunk regression: sky_light at chunk B (lx=8, local_y=4, lz=4) must be > 0 \
         after pull from Uniform(15) West neighbour; got {level}"
    );
}

// ── Two-pocket cross-chunk regression ─────────────────────────────────────
//
// Geometry (two chunks, same setup as the lower-pocket case but the dark
// column has TWO air pockets):
//
//   Chunk B, column (lx=8, lz=4):
//     local_y=0..3  solid
//     local_y=4     AIR  ← lower pocket (same as the single-pocket case)
//     local_y=5..12 solid
//     local_y=13    AIR  ← upper pocket
//     local_y=14..15 solid
//
//   To let the BFS reach lx=8 at local_y=13 via the West face pull, the
//   horizontal path at local_y=13 must also be open.  Make lx=0..8 air at
//   local_y=13, lz=4 in chunk B.  (The path could also be reached via an
//   adjacent column in the same chunk seeding at local_y=13, but the
//   horizontal-path approach is the cleaner direct test.)
//
//   After the fix (seeding all y levels for both kinds of Case-B columns),
//   the pull from Uniform(15) chunk A injects level 14 at every face cell of
//   chunk B's West face, including y=13; the BFS propagates from (lx=0,
//   local_y=13) → (lx=8, local_y=13).
//
// Expected: sky_light > 0 at both (lx=8, local_y=4, lz=4) AND
//           (lx=8, local_y=13, lz=4) in chunk B.
#[test]
fn cross_chunk_uniform_neighbour_lights_both_air_pockets() {
    let (mut app, dim) = make_test_app();

    let chunk_a_pos = ChunkPos::new(0, 4, 0);
    let chunk_b_pos = ChunkPos::new(1, 4, 0);

    let palette_a = all_air_palette();

    let base_bx = chunk_b_pos.x * 16;
    let base_by = chunk_b_pos.y * 16;
    let base_bz = chunk_b_pos.z * 16;
    let mut palette_b = BlockPalette::default();
    palette_b.fill(STONE);

    // Horizontal tunnel at local_y=4, lz=4 across the full chunk width.
    for lx in 0..16i32 {
        palette_b.set(BlockPos::new(base_bx + lx, base_by + 4, base_bz + 4), AIR);
    }
    // Horizontal tunnel at local_y=13, lz=4 from the West face to lx=8.
    for lx in 0..=8i32 {
        palette_b.set(BlockPos::new(base_bx + lx, base_by + 13, base_bz + 4), AIR);
    }

    let _chunk_a = spawn_chunk_with_palette(&mut app, dim, chunk_a_pos, palette_a);
    let chunk_b = spawn_chunk_with_palette(&mut app, dim, chunk_b_pos, palette_b);

    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level_lower = sky_level_at(&app, chunk_b, 8, 4, 4);
    assert!(
        level_lower > 0,
        "two-pocket regression (lower pocket): sky_light at (lx=8, local_y=4, lz=4) must be > 0; got {level_lower}"
    );

    let level_upper = sky_level_at(&app, chunk_b, 8, 13, 4);
    assert!(
        level_upper > 0,
        "two-pocket regression (upper pocket): sky_light at (lx=8, local_y=13, lz=4) must be > 0; got {level_upper}"
    );
}
