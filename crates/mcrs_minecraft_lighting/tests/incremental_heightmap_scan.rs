// Specification tests for the single-pass incremental top-down heightmap
// scan.
//
// The scan walks section slots top-down. For each XZ column it writes the
// heightmap (both world_surface and motion_blocking variants) the moment it
// finds the first qualifying block, and records closure in a per-variant
// 256-bit bitset. Finalization is fully derived (`is_finalized()`): the
// predicate fires when the cursor drops below `min_section_y`
// (chimney-to-bedrock) or both variants have closed every XZ column.
//
// Test geometry convention
// ─────────────────────────
// All tests use a small synthetic dimension so the logic is easy to trace
// without spawning 24 real sections. The dimension is
//   min_y = 0, height = 48  →  section_count = 3, min_section_y = 0
//   sections: y=0 (bottom), y=1 (middle), y=2 (top / max_section_y)
//
// The stub block-light table has three states:
//   state 0 = AIR    (dampening=0, IS_NOT_AIR clear, PROPAGATES_SKYLIGHT_DOWN set)
//   state 1 = STONE  (dampening=15, IS_NOT_AIR | IS_SOLID_OPAQUE | IS_MOTION_BLOCKING)
//   state 2 = LEAVES (dampening=1,  IS_NOT_AIR only — surface-blocking but
//                                   not motion-blocking)

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ColumnChunks, ColumnPlugin, Heightmaps, InColumn};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::components::IsAllAir;
use mcrs_minecraft_lighting::lifecycle::ColumnHeightmapScan;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_protocol::BlockStateId;

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);
const LEAVES: BlockStateId = BlockStateId(2);

// Three-section dimension: sections at chunk_y ∈ {0, 1, 2}, max_section_y = 2.
const DIM_MIN_Y: i32 = 0;
const DIM_HEIGHT: u32 = 48;

fn make_test_app() -> (App, Entity) {
    make_test_app_with_dim(DIM_MIN_Y, DIM_HEIGHT)
}

fn make_test_app_with_dim(min_y: i32, height: u32) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_table());
    let dim = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(min_y, height),
            dimension_id: DimensionId::new("test:incremental"),
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
    // AIR
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    // STONE
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    // LEAVES: blocks world_surface but NOT motion_blocking.
    emission[2] = 0;
    dampening[2] = 1;
    flags[2] = flag_bits::IS_NOT_AIR;
    BlockLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn spawn_section(app: &mut App, dim: Entity, chunk_y: i32, palette: BlockPalette) -> Entity {
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

fn column_entity(app: &App, section: Entity) -> Entity {
    app.world()
        .get::<InColumn>(section)
        .expect("InColumn back-link missing")
        .0
}

fn scan_state(app: &App, col: Entity) -> &ColumnHeightmapScan {
    app.world()
        .get::<ColumnHeightmapScan>(col)
        .expect("ColumnHeightmapScan missing on column entity")
}

// ─────────────────────────────────────────────────────────────────────────
// T1: top_down_scan_finalizes_after_surface_section_only
// ─────────────────────────────────────────────────────────────────────────
//
// Geometry: 3-section dim (chunk_y ∈ {0,1,2}). Only the top section (y=2)
// is spawned, with all-stone terrain. The scan starts at max_section_y=2,
// finds it loaded, scans it — all 256 XZ columns hit STONE at cell_y=15,
// so both bitsets become full at the first row. Finalization fires.
// ChunkNeedsInitialLight is inserted on the top section (and consumed by
// seed_initial_light in the same tick's Enqueue stage). Sections y=0 and
// y=1 are absent and must NOT receive ChunkNeedsInitialLight.
#[test]
fn top_down_scan_finalizes_after_surface_section_only() {
    let (mut app, dim) = make_test_app();

    let top_section = spawn_section(&mut app, dim, 2, all_stone());

    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    let scan = scan_state(&app, col);

    assert!(
        scan.is_finalized(),
        "scan must finalize after topmost section loaded with a complete stone surface"
    );

    // Heightmap must report surface = 48 (world_y=47 + 1) for all XZ.
    let h = app.world().get::<Heightmaps>(col).unwrap();
    for z in 0..16usize {
        for x in 0..16usize {
            assert_eq!(
                h.surface_get(x, z),
                48,
                "surface at ({x},{z}) must be 48 after all-stone top section"
            );
            assert_eq!(
                h.motion_blocking_get(x, z),
                48,
                "motion_blocking at ({x},{z}) must be 48 after all-stone top section"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T2: air_section_fast_skip
// ─────────────────────────────────────────────────────────────────────────
//
// Top section (y=2) is all air. Middle section (y=1) has stone everywhere.
// Bottom section (y=0) is absent.
//
// Scan starts at max=2: section y=2 is present, IsAllAir set → fast-skip
// (no per-block work, no closure). Cursor advances to 1. Section y=1 is
// present, not IsAllAir. First row scan finds opaque at every XZ →
// bitsets fill → finalized.
//
// Surface = section_base_y(y=1) + 15 + 1 = 32 for all XZ.
#[test]
fn air_section_fast_skip() {
    let (mut app, dim) = make_test_app();

    let top_section = spawn_section(&mut app, dim, 2, all_air());
    let _mid_section = spawn_section(&mut app, dim, 1, all_stone());

    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);

    assert!(
        app.world().get::<IsAllAir>(top_section).is_some(),
        "top section (all-air palette) must carry IsAllAir marker"
    );

    let scan = scan_state(&app, col);
    assert!(
        scan.is_finalized(),
        "scan must finalize: air top section skipped, stone middle section closes all columns"
    );

    let h = app.world().get::<Heightmaps>(col).unwrap();
    for z in 0..16usize {
        for x in 0..16usize {
            assert_eq!(
                h.surface_get(x, z),
                32,
                "surface at ({x},{z}) must be 32 after all-stone section at chunk_y=1"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T3: out_of_order_arrival_completes_scan
// ─────────────────────────────────────────────────────────────────────────
//
// Sections arrive across ticks:
//   Tick 1: spawn top (y=2, all-air).
//   Tick 2: spawn bottom (y=0, all-stone) — NOT the middle (y=1).
//   Tick 3: spawn middle (y=1, all-stone).
//
// After tick 1: scan at cursor=2, section y=2 loaded → fast-skip. Cursor
// advances to 1. Section y=1 absent → pause. NOT finalized.
//
// After tick 2: Changed<ColumnChunks> fires (y=0 registered). Cursor is at
// 1. Section y=1 still absent → still no progress. NOT finalized.
//
// After tick 3: Changed<ColumnChunks> fires (y=1 registered). Cursor is at
// 1. Section y=1 now loaded, all-stone → scans, bitsets fill → Finalized.
#[test]
fn out_of_order_arrival_completes_scan() {
    let (mut app, dim) = make_test_app();

    // Tick 1: top section (all air).
    let top_section = spawn_section(&mut app, dim, 2, all_air());
    app.world_mut().run_schedule(FixedUpdate);

    {
        let col = column_entity(&app, top_section);
        let scan = scan_state(&app, col);
        assert!(
            !scan.is_finalized(),
            "must NOT finalize after only top (air) section with y=1 still absent"
        );
    }

    // Tick 2: bottom section arrives — still missing the middle.
    let _bottom_section = spawn_section(&mut app, dim, 0, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    {
        let col = column_entity(&app, top_section);
        let scan = scan_state(&app, col);
        assert!(
            !scan.is_finalized(),
            "must NOT finalize when middle section (y=1) is still absent"
        );
    }

    // Tick 3: middle section arrives — scan can now advance past y=1.
    let _mid_section = spawn_section(&mut app, dim, 1, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    let scan = scan_state(&app, col);
    assert!(
        scan.is_finalized(),
        "scan must finalize once the blocking middle section (y=1) arrives"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// T4: chimney_to_bedrock_finalizes_at_floor
// ─────────────────────────────────────────────────────────────────────────
//
// All three sections fully air. After all three arrive, the scan walks
// y=2 (air, skip), y=1 (air, skip), y=0 (air, skip), then scan_cursor
// falls below min_section_y (0). The bedrock-floor termination fires.
// is_finalized() returns true; the bitsets remain all-zero (no opaque
// block was found — the bitsets are the closure source-of-truth, so
// "all-zero AND finalized" unambiguously means chimney-to-bedrock).
//
// Heightmap surface = min_y = 0 for all XZ (chimney from top to bedrock).
// advance_scan writes the sentinel explicitly along the chimney path.
#[test]
fn chimney_to_bedrock_finalizes_at_floor() {
    let (mut app, dim) = make_test_app();

    let top_section = spawn_section(&mut app, dim, 2, all_air());
    let _mid_section = spawn_section(&mut app, dim, 1, all_air());
    let _bot_section = spawn_section(&mut app, dim, 0, all_air());

    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    let col = column_entity(&app, top_section);
    let scan = scan_state(&app, col);
    assert!(
        scan.is_finalized(),
        "scan must finalize at bedrock floor when all sections are air"
    );
    // Chimney path indicator: cursor dropped below min_section_y.
    assert!(
        scan.scan_cursor < 0,
        "chimney-to-bedrock: scan_cursor must drop below min_section_y=0 (got {})",
        scan.scan_cursor
    );

    let h = app.world().get::<Heightmaps>(col).unwrap();
    for z in 0..16usize {
        for x in 0..16usize {
            assert_eq!(
                h.surface_get(x, z),
                DIM_MIN_Y,
                "chimney heightmap at ({x},{z}) must equal min_y={DIM_MIN_Y}"
            );
            assert_eq!(
                h.motion_blocking_get(x, z),
                DIM_MIN_Y,
                "chimney motion_blocking at ({x},{z}) must equal min_y={DIM_MIN_Y}"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T5: late_arriving_section_gets_initial_light_immediately
// ─────────────────────────────────────────────────────────────────────────
//
// Spawn top (y=2, all-stone) → scan finalizes on tick 1. Then in tick 2
// spawn middle (y=1) — it must receive ChunkNeedsInitialLight immediately
// (consumed by seed_initial_light in the same tick), so SkyLight ends up
// populated.
#[test]
fn late_arriving_section_gets_initial_light_immediately() {
    let (mut app, dim) = make_test_app();

    let top_section = spawn_section(&mut app, dim, 2, all_stone());

    app.world_mut().run_schedule(FixedUpdate);

    {
        let col = column_entity(&app, top_section);
        assert!(
            scan_state(&app, col).is_finalized(),
            "scan must finalize after all-stone top section"
        );
    }

    let mid_section = spawn_section(&mut app, dim, 1, all_air());
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    assert!(
        scan_state(&app, col).is_finalized(),
        "finalized state must remain after late section arrival"
    );
    use mcrs_minecraft_lighting::components::SkyLight;
    assert!(
        app.world().get::<SkyLight>(mid_section).is_some(),
        "late-arriving section must have SkyLight component attached"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// T6: partial_column_heightmap_is_sentinel_until_finalized
// ─────────────────────────────────────────────────────────────────────────
//
// Pins the partial-column contract: until the scan finalizes, entries for
// XZ columns the scan has not yet closed hold the sentinel value (min_y).
//
// Step 1: spawn only a lower section. Scan cursor pauses at the topmost
// missing slot. Every XZ reads back as min_y.
// Step 2: spawn the topmost section with a complete stone surface. Scan
// finalizes. Every XZ now reads back the surface value.
#[test]
fn partial_column_heightmap_is_sentinel_until_finalized() {
    let (mut app, dim) = make_test_app();

    // Step 1: only a lower section.
    let bottom_section = spawn_section(&mut app, dim, 0, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, bottom_section);
    assert!(
        !scan_state(&app, col).is_finalized(),
        "scan must NOT finalize while the topmost section is absent"
    );
    {
        let h = app.world().get::<Heightmaps>(col).unwrap();
        for &(x, z) in &[(0usize, 0usize), (3, 7), (15, 15)] {
            assert_eq!(
                h.surface_get(x, z),
                DIM_MIN_Y,
                "partial column surface at ({x},{z}) must equal sentinel min_y"
            );
            assert_eq!(
                h.motion_blocking_get(x, z),
                DIM_MIN_Y,
                "partial column motion_blocking at ({x},{z}) must equal sentinel min_y"
            );
        }
    }

    // Step 2: spawn the topmost section with a stone surface row.
    let _top_section = spawn_section(&mut app, dim, 2, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    assert!(
        scan_state(&app, col).is_finalized(),
        "scan must finalize after the topmost stone section arrives"
    );
    let h = app.world().get::<Heightmaps>(col).unwrap();
    for z in 0..16usize {
        for x in 0..16usize {
            assert_eq!(
                h.surface_get(x, z),
                48,
                "finalized surface at ({x},{z}) must be 48"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T7: multi_variant_divergence
// ─────────────────────────────────────────────────────────────────────────
//
// Constructs a column where one specific XZ has LEAVES at cell_y=8 and
// STONE at cell_y=4 in the top section, while every other XZ has STONE
// at cell_y=15. LEAVES set IS_NOT_AIR but not IS_MOTION_BLOCKING — so
// world_surface closes one row earlier than motion_blocking at the
// special XZ. After finalization:
//   - At the special XZ: surface = section_base_y + 9, motion = section_base_y + 5.
//   - At every other XZ: surface = motion = section_base_y + 16.
// Pins the independence of the two heightmap variants.
#[test]
fn multi_variant_divergence() {
    let (mut app, dim) = make_test_app();

    let mut top_palette = BlockPalette::default();
    top_palette.fill(AIR);
    // Top row stone for every XZ except (5, 5).
    for z in 0..16i32 {
        for x in 0..16i32 {
            if (x, z) != (5, 5) {
                top_palette.set(BlockPos::new(x, 15, z), STONE);
            }
        }
    }
    // At (5, 5): leaves higher than stone.
    top_palette.set(BlockPos::new(5, 8, 5), LEAVES);
    top_palette.set(BlockPos::new(5, 4, 5), STONE);

    let top_section = spawn_section(&mut app, dim, 2, top_palette);
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    let scan = scan_state(&app, col);
    assert!(scan.is_finalized(), "scan must finalize on top-section row");

    let h = app.world().get::<Heightmaps>(col).unwrap();
    let section_base_y = 32; // chunk_y=2 * 16

    // Special XZ: surface from leaves, motion_blocking from stone below.
    assert_eq!(
        h.surface_get(5, 5),
        section_base_y + 9,
        "surface at the leaf column must be one above the leaves (cell_y=8)"
    );
    assert_eq!(
        h.motion_blocking_get(5, 5),
        section_base_y + 5,
        "motion_blocking at the leaf column must be one above the stone (cell_y=4)"
    );

    // Other XZs: both variants close on the top row.
    for &(x, z) in &[(0usize, 0usize), (1, 1), (15, 15)] {
        assert_eq!(h.surface_get(x, z), section_base_y + 16);
        assert_eq!(h.motion_blocking_get(x, z), section_base_y + 16);
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T8: heightmap_updates_correctly_after_block_break_post_finalize
// ─────────────────────────────────────────────────────────────────────────
//
// Pins the contract that `update_heightmaps_on_block_placed` remains the
// authoritative writer for post-finalization heightmap edits and is not
// interfered with by the scan.
//
// Adapted to the 3-section test dim:
//   1. Top section has stone at cell_y=14 for ALL XZ (creating surface at
//      world_y=47 for all). Additional stone at (0, 12, 0) (world_y=44)
//      provides a next-lower opaque block at XZ=(0,0).
//   2. Scan finalizes on tick 1 — every XZ closes on cell_y=14.
//      Expected: surface=47 at (0,0).
//   3. Break (0, 46, 0): emit BlockPlaced(old=STONE, new=AIR). The
//      rescan in update_heightmaps_on_block_placed finds the stone at
//      (0, 44, 0) → surface drops to 45.
#[test]
fn heightmap_updates_correctly_after_block_break_post_finalize() {
    let (mut app, dim) = make_test_app();

    let mut top_palette = BlockPalette::default();
    top_palette.fill(AIR);
    for z in 0..16i32 {
        for x in 0..16i32 {
            top_palette.set(BlockPos::new(x, 14, z), STONE);
        }
    }
    top_palette.set(BlockPos::new(0, 12, 0), STONE);

    let top_section = spawn_section(&mut app, dim, 2, top_palette);
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    assert!(
        scan_state(&app, col).is_finalized(),
        "scan must finalize after the full-row top section"
    );
    {
        let h = app.world().get::<Heightmaps>(col).unwrap();
        assert_eq!(h.surface_get(0, 0), 47, "pre-break surface at (0,0)");
    }

    // Break the topmost stone at (0, 46, 0).
    app.world_mut()
        .get_mut::<BlockPalette>(top_section)
        .expect("palette missing")
        .set(BlockPos::new(0, 14, 0), AIR);
    let placed = BlockPlaced {
        chunk: top_section,
        chunk_pos: ChunkPos::new(0, 2, 0),
        block_pos: BlockPos::new(0, 46, 0),
        old_state: STONE,
        new_state: AIR,
        flags: BlockUpdateFlags::all(),
    };
    app.world_mut()
        .resource_mut::<Messages<BlockPlaced>>()
        .write(placed);
    app.world_mut().run_schedule(FixedUpdate);

    let h = app.world().get::<Heightmaps>(col).unwrap();
    assert_eq!(
        h.surface_get(0, 0),
        45,
        "post-break surface at (0,0) must drop to the next-lower opaque + 1"
    );
    // Untouched columns must not change.
    assert_eq!(
        h.surface_get(1, 0),
        47,
        "untouched columns must keep their pre-break surface"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// T9: re_trigger_changed_without_new_sections_is_noop
// ─────────────────────────────────────────────────────────────────────────
//
// After the scan finalizes, spuriously marking `ColumnChunks` as changed
// (without adding a slot) must not panic, must not corrupt the heightmap,
// and must leave the scan in its finalized state.
#[test]
fn re_trigger_changed_without_new_sections_is_noop() {
    let (mut app, dim) = make_test_app();

    let top_section = spawn_section(&mut app, dim, 2, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, top_section);
    assert!(
        scan_state(&app, col).is_finalized(),
        "scan must finalize on tick 1"
    );

    let surface_before: Vec<i32> = {
        let h = app.world().get::<Heightmaps>(col).unwrap();
        (0..256)
            .map(|i| h.surface_get(i & 15, i >> 4))
            .collect()
    };

    // Touch ColumnChunks to mark it changed without adding a slot.
    {
        let mut col_chunks = app
            .world_mut()
            .get_mut::<ColumnChunks>(col)
            .expect("ColumnChunks missing");
        col_chunks.set_changed();
    }
    app.world_mut().run_schedule(FixedUpdate);

    let scan = scan_state(&app, col);
    assert!(
        scan.is_finalized(),
        "scan must stay finalized after spurious Changed event"
    );

    let h = app.world().get::<Heightmaps>(col).unwrap();
    for i in 0..256 {
        let x = i & 15;
        let z = i >> 4;
        assert_eq!(
            h.surface_get(x, z),
            surface_before[i],
            "heightmap must be unchanged after spurious Changed at ({x},{z})"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// T10: single_section_dimension_finalizes_immediately
// ─────────────────────────────────────────────────────────────────────────
//
// A one-section dimension (min_section_y == max_section_y). The single
// section IS the topmost. Spawning it must finalize the scan in this
// single event and insert ChunkNeedsInitialLight on the section.
#[test]
fn single_section_dimension_finalizes_immediately() {
    let (mut app, dim) = make_test_app_with_dim(0, 16);

    let only_section = spawn_section(&mut app, dim, 0, all_stone());
    app.world_mut().run_schedule(FixedUpdate);

    let col = column_entity(&app, only_section);
    assert!(
        scan_state(&app, col).is_finalized(),
        "single-section dim must finalize on the first event"
    );

    use mcrs_minecraft_lighting::components::SkyLight;
    assert!(
        app.world().get::<SkyLight>(only_section).is_some(),
        "the only section must have SkyLight attached"
    );

    let h = app.world().get::<Heightmaps>(col).unwrap();
    for z in 0..16usize {
        for x in 0..16usize {
            assert_eq!(
                h.surface_get(x, z),
                16,
                "single-section dim surface at ({x},{z}) must be 16"
            );
        }
    }
}
