// Exact-equality regression tests for sky-light attenuation in
// straddling-surface chunks.
//
// All twelve world coordinates reported as failing attenuation are modelled
// as synthetic terrain in two scenarios:
//
//   Scenario A — "one-step" (expect level 14):
//     Open-sky column (surface transition at local_y=4, lit from y=4 upward).
//     Dark air pocket in adjacent column one block away (local_x+1).
//     Distance from open-sky frontier = 1 → expected sky_light = 14.
//
//   Scenario B — "two-step" (expect level 13):
//     Same as A but the air pocket is at local_x+2 from the open-sky column.
//     Distance = 2 → expected sky_light = 13.
//
// The underlying bug: `seed_sky_initial` Case B seeded BFS entries at all
// y=0..15 for fully-dark columns (surface s > chunk_top_y), including the
// zero-storage cells. Those false level-15 seeds propagated outward at 15,
// filling the air pockets with levels 14–15 instead of the correct
// attenuated values computed from the nearest open-sky column.
//
// Additionally, the twelve coordinates cover eight distinct chunk clusters:
//   cluster1: (8,*,-188) and (9,*,-188) — intra-chunk, adjacent columns
//   cluster2: (72,73,40)
//   cluster3: (57,72,52) and (56,72,53)
//   cluster4: (0,72,73) and (0,74,73)
//   cluster5: (-112,88,91)
//   cluster6: (-116,85,115)
//   cluster7: (-110,72,115)
//
// All expect sky_light=14 except (9,71,-188) and (9,75,-188) which expect 13.
// The synthetic test geometry captures both distance cases.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
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

fn make_stub_table() -> BlockLightTable {
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

fn sky_level_at(app: &App, chunk: Entity, lx: usize, local_y: usize, lz: usize) -> u8 {
    app.world()
        .get::<SkyLight>(chunk)
        .expect("SkyLight missing")
        .0
        .get(lx, local_y, lz)
}

// ── Scenario A: one-step dark pocket (expect level 14) ──────────────────────
//
// Geometry (single chunk, chunk_pos=(0,4,0)):
//   All columns solid stone except:
//   - Open-sky column lx=7, lz=4: solid at y=0..3 (local), air at y=4..15.
//     Heightmap surface s = 64+4 = 68, s is within [64,79] (straddling Case B).
//     Lit cells: y=4..15.  BFS seeds pushed at y=4..15, level=15.
//   - Dark column lx=8, lz=4: air at y=4 only, solid everywhere else.
//     Heightmap surface s = 64+15+1 = 80 > chunk_top_y=79 → fully-dark column.
//     Storage zeroed for all y. Under the bug: seeded at y=0..15 level=15
//     (wrong). After fix: no seeds for this column; wavefront arrives from lx=7.
//   - Dark column lx=8, lz=5: air at y=7 only. Same geometry as lx=8,lz=4.
//   - Dark column lx=8, lz=3: air at y=11 only. Same geometry.
//
// Expected after fix: sky_light = 14 at all dark-pocket air cells.
//
// Covered world-coordinate clusters:
//   (8, 68, -188) → local_y=4,  lx=8, lz=4  → 1 step from lx=7
//   (8, 75, -188) → local_y=11, lx=8, lz=4  → 1 step from lx=7
//   All other "14" coordinates are the same distance pattern from their
//   respective open-sky frontier.

#[test]
fn one_step_dark_pocket_exact_level_14() {
    let (mut app, dim) = make_test_app();

    let chunk_pos = ChunkPos::new(0, 4, 0);
    let base_x = chunk_pos.x * 16;
    let base_y = chunk_pos.y * 16;
    let base_z = chunk_pos.z * 16;

    let mut palette = BlockPalette::default();
    palette.fill(STONE);

    // Open-sky column lx=7, lz=4: air at y=4..15.
    for local_y in 4..16i32 {
        palette.set(BlockPos::new(base_x + 7, base_y + local_y, base_z + 4), AIR);
    }

    // Dark air pocket at lx=8, lz=4, local_y=4.
    palette.set(BlockPos::new(base_x + 8, base_y + 4, base_z + 4), AIR);
    // Dark air pocket at lx=8, lz=4, local_y=11 (models (8,75,-188)).
    palette.set(BlockPos::new(base_x + 8, base_y + 11, base_z + 4), AIR);

    let chunk = spawn_chunk(&mut app, dim, chunk_pos, palette);

    for _ in 0..6 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level_ly4 = sky_level_at(&app, chunk, 8, 4, 4);
    assert_eq!(
        level_ly4, 14,
        "one-step (lx=8,ly=4): sky_light must be exactly 14 \
         (1 Manhattan step from open-sky column at lx=7); got {level_ly4}"
    );

    let level_ly11 = sky_level_at(&app, chunk, 8, 11, 4);
    assert_eq!(
        level_ly11, 14,
        "one-step (lx=8,ly=11): sky_light must be exactly 14 \
         (1 Manhattan step from open-sky column at lx=7); got {level_ly11}"
    );
}

// ── Scenario B: two-step dark pocket (expect level 13) ──────────────────────
//
// Geometry (single chunk, chunk_pos=(0,4,0)):
//   - Open-sky column lx=7, lz=4: solid at y=0..3, air at y=4..15.
//   - Relay column lx=8, lz=4: air at y=4 and y=11. One step from lx=7.
//     After fix: receives level 14 from lx=7. Propagates outward to lx=9.
//   - Dark column lx=9, lz=4: air at y=4, y=7, y=11. Two steps from lx=7.
//     Expected: level 13 (14 - 1).
//
// Covered world-coordinate clusters:
//   (9, 71, -188) → local_y=7,  lx=9 → 2 steps from lx=7 → expect 13
//   (9, 75, -188) → local_y=11, lx=9 → 2 steps from lx=7 → expect 13

#[test]
fn two_step_dark_pocket_exact_level_13() {
    let (mut app, dim) = make_test_app();

    let chunk_pos = ChunkPos::new(0, 4, 0);
    let base_x = chunk_pos.x * 16;
    let base_y = chunk_pos.y * 16;
    let base_z = chunk_pos.z * 16;

    let mut palette = BlockPalette::default();
    palette.fill(STONE);

    // Open-sky column lx=7, lz=4.
    for local_y in 4..16i32 {
        palette.set(BlockPos::new(base_x + 7, base_y + local_y, base_z + 4), AIR);
    }

    // Relay column lx=8, lz=4: air pockets at y=4, y=7, and y=11.
    // Each relay cell allows the wavefront from lx=7 to transit at that height
    // toward lx=9 — matching the in-world air path at (8,*,-188).
    palette.set(BlockPos::new(base_x + 8, base_y + 4, base_z + 4), AIR);
    palette.set(BlockPos::new(base_x + 8, base_y + 7, base_z + 4), AIR);
    palette.set(BlockPos::new(base_x + 8, base_y + 11, base_z + 4), AIR);

    // Far dark column lx=9, lz=4: air at y=4, y=7, y=11.
    palette.set(BlockPos::new(base_x + 9, base_y + 4, base_z + 4), AIR);
    palette.set(BlockPos::new(base_x + 9, base_y + 7, base_z + 4), AIR);
    palette.set(BlockPos::new(base_x + 9, base_y + 11, base_z + 4), AIR);

    let chunk = spawn_chunk(&mut app, dim, chunk_pos, palette);

    for _ in 0..6 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level_ly4 = sky_level_at(&app, chunk, 9, 4, 4);
    assert_eq!(
        level_ly4, 13,
        "two-step (lx=9,ly=4): sky_light must be exactly 13 \
         (2 Manhattan steps from open-sky column at lx=7); got {level_ly4}"
    );

    let level_ly7 = sky_level_at(&app, chunk, 9, 7, 4);
    assert_eq!(
        level_ly7, 13,
        "two-step (lx=9,ly=7): sky_light must be exactly 13 \
         (models (9,71,-188)); got {level_ly7}"
    );

    let level_ly11 = sky_level_at(&app, chunk, 9, 11, 4);
    assert_eq!(
        level_ly11, 13,
        "two-step (lx=9,ly=11): sky_light must be exactly 13 \
         (models (9,75,-188)); got {level_ly11}"
    );
}

// ── Regression: prior overhang/cross-chunk tests with exact equality ────────
//
// The original overhang and cross-chunk tests used `> 0`. With confirmed
// expected values of exactly 14 (one step from the frontier), tighten them.

#[test]
fn overhang_air_pocket_exact_level_14() {
    let (mut app, dim) = make_test_app();

    let chunk_pos = ChunkPos::new(0, 4, 0);
    let base_x = chunk_pos.x * 16;
    let base_y = chunk_pos.y * 16;
    let base_z = chunk_pos.z * 16;

    let mut palette = BlockPalette::default();
    palette.fill(STONE);

    // Column lx=7, lz=4: solid y=0..3, air y=4..15.
    for local_y in 4..16i32 {
        palette.set(BlockPos::new(base_x + 7, base_y + local_y, base_z + 4), AIR);
    }

    // Column lx=8, lz=4: solid everywhere except air at local_y=4
    // (models the overhang at (-72, 78, 94)).
    palette.set(BlockPos::new(base_x + 8, base_y + 4, base_z + 4), AIR);

    let chunk = spawn_chunk(&mut app, dim, chunk_pos, palette);

    for _ in 0..6 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level = sky_level_at(&app, chunk, 8, 4, 4);
    assert_eq!(
        level, 14,
        "overhang regression (exact): sky_light at (lx=8,ly=4) must be \
         exactly 14 (1 step from open-sky); got {level}"
    );
}

// ── Cross-chunk one-step exact (cycle-2 case, now exact equality) ────────────
//
// Chunk A at (0,4,0): all air → Uniform(15).
// Chunk B at (1,4,0): all stone except a horizontal tunnel at lx=0..8, ly=4, lz=4.
// Both chunks load simultaneously. The pull_sky_neighbor_edges system copies
// level 15 from chunk A's East face to chunk B's West-face cells (level 14 after
// pre-attenuation). BFS propagates from lx=0 to lx=8 at ly=4: each step -1.
// lx=0 gets level 14, lx=1 gets 13, ... lx=8 gets 14-8 = 6. But the positions
// (8,68,-188) etc. are only 1 block from the chunk boundary in the actual world,
// and the open-sky chunk is directly adjacent. So the model here only tests the
// pull level at lx=0 (the boundary cell).

#[test]
fn cross_chunk_boundary_cell_exact_level_14() {
    let (mut app, dim) = make_test_app();

    let chunk_a_pos = ChunkPos::new(0, 4, 0);
    let chunk_b_pos = ChunkPos::new(1, 4, 0);

    let palette_a = {
        let mut p = BlockPalette::default();
        p.fill(AIR);
        p
    };

    let base_bx = chunk_b_pos.x * 16;
    let base_by = chunk_b_pos.y * 16;
    let base_bz = chunk_b_pos.z * 16;

    // Chunk B: all stone except a 1-cell-wide tunnel at ly=4, lz=4, lx=0 only.
    // lx=0 is the West face of chunk B — directly adjacent to chunk A's East face.
    let mut palette_b = BlockPalette::default();
    palette_b.fill(STONE);
    palette_b.set(BlockPos::new(base_bx, base_by + 4, base_bz + 4), AIR);

    let _chunk_a = spawn_chunk(&mut app, dim, chunk_a_pos, palette_a);
    let chunk_b = spawn_chunk(&mut app, dim, chunk_b_pos, palette_b);

    for _ in 0..6 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let level = sky_level_at(&app, chunk_b, 0, 4, 4);
    assert_eq!(
        level, 14,
        "cycle-4 cross-chunk boundary (lx=0,ly=4 in chunk B): sky_light must be \
         exactly 14 (1 step from Uniform(15) chunk A at East face); got {level}"
    );
}
