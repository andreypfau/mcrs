// Regression coverage for the unified `scan_top_down` shared core that
// powers both `lifecycle::advance_scan` and `heightmap_update::rescan_column_xz`.
//
// The core walks `ColumnChunks` top-down, evaluates a per-cell predicate
// against `BlockLightTable::flags_for`, and fires `on_closed(x, z, variant,
// world_y)` exactly once per (x, z, variant) pair. The shared body keeps
// both call sites byte-identical to the pre-refactor scanners — the
// fixtures below pin the documented behaviours so any future bug in the
// shared core surfaces here even when the broader integration suite stays
// green.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ColumnPlugin, Heightmaps};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::lifecycle::ColumnHeightmapScan;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_protocol::BlockStateId;

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);
const LEAVES: BlockStateId = BlockStateId(2);

const DIM_MIN_Y: i32 = 0;
const DIM_HEIGHT: u32 = 48;
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
            type_config: DimensionTypeConfig::new(DIM_MIN_Y, DIM_HEIGHT),
            dimension_id: DimensionId::new("test:scanner_unification"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(dim).insert(HasSkyLight);
    (app, dim)
}

fn make_stub_table() -> BlockLightTable {
    let state_count = 3usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    dampening[1] = 15;
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    dampening[2] = 1;
    flags[2] = flag_bits::IS_NOT_AIR;
    let _ = emission.iter_mut();
    BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn spawn_chunk(app: &mut App, dim: Entity, chunk_y: i32, palette: BlockPalette) -> Entity {
    app.world_mut()
        .spawn((
            InDimension(dim),
            ChunkPos::new(0, chunk_y, 0),
            ChunkEntities::default(),
            Chunk,
            ChunkLoaded,
            palette,
        ))
        .id()
}

fn all_air() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(AIR);
    p
}

fn all_stone() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(STONE);
    p
}

/// Find the unique column entity carrying `Heightmaps` and return its
/// `Entity` id.
fn unique_column(app: &mut App) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &Heightmaps)>();
    let mut hits = q.iter(app.world()).map(|(e, _)| e).collect::<Vec<_>>();
    assert_eq!(hits.len(), 1, "expected exactly one column with Heightmaps");
    hits.pop().unwrap()
}

fn read_heightmap_pairs(app: &App, col: Entity) -> [(i32, i32); 256] {
    let hm = app.world().get::<Heightmaps>(col).expect("Heightmaps on column");
    let mut out = [(0i32, 0i32); 256];
    for z in 0..16usize {
        for x in 0..16usize {
            let idx = (z << 4) | x;
            out[idx] = (hm.surface_get(x, z), hm.motion_blocking_get(x, z));
        }
    }
    out
}

fn run_to_finalized(app: &mut App, max_ticks: usize) {
    for _ in 0..max_ticks {
        app.world_mut().run_schedule(FixedUpdate);
        let mut q = app
            .world_mut()
            .query::<&ColumnHeightmapScan>();
        let finalized = q.iter(app.world()).any(|scan| scan.is_finalized());
        if finalized {
            return;
        }
    }
    panic!("scan did not finalize within {max_ticks} ticks");
}

#[test]
fn all_stone_closes_surface_and_motion_at_top_chunk() {
    let (mut app, dim) = make_test_app();
    spawn_chunk(&mut app, dim, 2, all_stone());
    spawn_chunk(&mut app, dim, 1, all_stone());
    spawn_chunk(&mut app, dim, 0, all_stone());
    run_to_finalized(&mut app, 4);
    let col = unique_column(&mut app);

    let pairs = read_heightmap_pairs(&app, col);
    // Top chunk is chunk_y=2 spanning world y in 32..48. Topmost solid cell
    // is at world_y=47, stored as 48 ("Y+1") = the empty cell above. Both
    // variants close at the same cell because STONE sets both
    // IS_NOT_AIR and IS_MOTION_BLOCKING.
    for (idx, (surface, motion)) in pairs.iter().enumerate() {
        let x = idx & 15;
        let z = idx >> 4;
        assert_eq!(
            (*surface, *motion),
            (48, 48),
            "fixture all_stone: (x={x}, z={z}) expected (48, 48) got ({surface}, {motion})"
        );
    }
}

#[test]
fn all_air_finalizes_at_chimney_to_bedrock_with_sentinel() {
    let (mut app, dim) = make_test_app();
    spawn_chunk(&mut app, dim, 2, all_air());
    spawn_chunk(&mut app, dim, 1, all_air());
    spawn_chunk(&mut app, dim, 0, all_air());
    run_to_finalized(&mut app, 4);
    let col = unique_column(&mut app);

    let pairs = read_heightmap_pairs(&app, col);
    // Chimney-to-bedrock: every (x, z) gets the min_y sentinel for both
    // variants. min_y = DIM_MIN_Y = 0 here.
    for (idx, (surface, motion)) in pairs.iter().enumerate() {
        let x = idx & 15;
        let z = idx >> 4;
        assert_eq!(
            (*surface, *motion),
            (DIM_MIN_Y, DIM_MIN_Y),
            "fixture all_air: (x={x}, z={z}) expected sentinel ({DIM_MIN_Y}, {DIM_MIN_Y}) got ({surface}, {motion})"
        );
    }
}

#[test]
fn leaves_over_stone_diverges_surface_from_motion() {
    let (mut app, dim) = make_test_app();

    // Top chunk: per-cell mix. Place LEAVES at every (x, 7, z) and STONE at
    // every (x, 3, z), AIR elsewhere. Both variants must walk the same
    // top-down path inside this chunk: surface closes at LEAVES (y=2*16+7=39
    // stored as 40), motion closes at STONE (y=2*16+3=35 stored as 36).
    let mut top = BlockPalette::default();
    top.fill(AIR);
    for z in 0..16i32 {
        for x in 0..16i32 {
            top.set((x, 7, z), LEAVES);
            top.set((x, 3, z), STONE);
        }
    }
    spawn_chunk(&mut app, dim, 2, top);
    spawn_chunk(&mut app, dim, 1, all_air());
    spawn_chunk(&mut app, dim, 0, all_air());
    run_to_finalized(&mut app, 4);
    let col = unique_column(&mut app);

    let pairs = read_heightmap_pairs(&app, col);
    for (idx, (surface, motion)) in pairs.iter().enumerate() {
        let x = idx & 15;
        let z = idx >> 4;
        assert_eq!(
            (*surface, *motion),
            (40, 36),
            "fixture leaves_over_stone: (x={x}, z={z}) expected (40, 36) got ({surface}, {motion})"
        );
    }
}

#[test]
fn surface_in_lower_chunks_with_air_above_closes_correctly() {
    let (mut app, dim) = make_test_app();
    // Top two chunks all-air, bottom chunk all-stone. Top of bottom chunk is
    // world y=15, stored as 16.
    spawn_chunk(&mut app, dim, 2, all_air());
    spawn_chunk(&mut app, dim, 1, all_air());
    spawn_chunk(&mut app, dim, 0, all_stone());
    run_to_finalized(&mut app, 6);
    let col = unique_column(&mut app);

    let pairs = read_heightmap_pairs(&app, col);
    for (idx, (surface, motion)) in pairs.iter().enumerate() {
        let x = idx & 15;
        let z = idx >> 4;
        assert_eq!(
            (*surface, *motion),
            (16, 16),
            "fixture stone_at_bottom: (x={x}, z={z}) expected (16, 16) got ({surface}, {motion})"
        );
    }
}

#[test]
fn lone_column_among_air_closes_only_that_column() {
    let (mut app, dim) = make_test_app();
    // Top chunk all-air, middle chunk has a single stone column at (5, *, 9),
    // bottom chunk all-air. Surface for (5, 9) closes at top of middle chunk
    // (world_y = 16 + 15 = 31, stored 32). Every other (x, z) reaches
    // chimney-to-bedrock with the sentinel.
    let mut mid = BlockPalette::default();
    mid.fill(AIR);
    for y in 0..16i32 {
        mid.set((5, y, 9), STONE);
    }
    spawn_chunk(&mut app, dim, 2, all_air());
    spawn_chunk(&mut app, dim, 1, mid);
    spawn_chunk(&mut app, dim, 0, all_air());
    run_to_finalized(&mut app, 6);
    let col = unique_column(&mut app);

    let pairs = read_heightmap_pairs(&app, col);
    for (idx, (surface, motion)) in pairs.iter().enumerate() {
        let x = idx & 15;
        let z = idx >> 4;
        let expected = if (x, z) == (5, 9) {
            (32, 32)
        } else {
            (DIM_MIN_Y, DIM_MIN_Y)
        };
        assert_eq!(
            (*surface, *motion),
            expected,
            "fixture lone_column: (x={x}, z={z}) expected {expected:?} got ({surface}, {motion})"
        );
    }
}

