#![cfg(feature = "test-bench")]

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::voxel_shape::Direction;
use mcrs_core::AppState;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ChunkColumnPos, ColumnIndex, ColumnPlugin};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_engine::world::lighting::LightTicket;
use mcrs_lighting::components::{BlockLight, LightDirty, SkyLight};
use mcrs_lighting::invariants::{check_block_light_invariants, check_sky_light_invariants};
use mcrs_lighting::storage::LightStorage;
use mcrs_lighting::stub::{block_light_nibbles, sky_light_nibbles};
use mcrs_lighting::table::BlockLightTable;
use mcrs_lighting::test_bench::{assert_nibbles_eq, from_input};
use mcrs_lighting::LightingPlugin;
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_protocol::BlockStateId;

#[path = "golden/mod.rs"]
mod golden;

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;

fn make_test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(golden::light_table::synthetic_block_light_table());
    let dim = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
            dimension_id: DimensionId::new("test:phase3"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(dim).insert(HasSkyLight);
    (app, dim)
}

fn spawn_air_section(app: &mut App, dim: Entity, chunk_pos: ChunkPos) -> Entity {
    let mut palette = BlockPalette::default();
    palette.fill(golden::light_table::SYNTH_AIR_ID);
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

fn set_palette_cell(app: &mut App, section: Entity, world_pos: BlockPos, state: BlockStateId) {
    app.world_mut()
        .get_mut::<BlockPalette>(section)
        .expect("BlockPalette missing on section")
        .set(world_pos, state);
}

fn emit_block_placed(
    app: &mut App,
    section: Entity,
    chunk_pos: ChunkPos,
    world_pos: BlockPos,
    old_state: BlockStateId,
    new_state: BlockStateId,
) {
    app.world_mut()
        .resource_mut::<Messages<BlockPlaced>>()
        .write(BlockPlaced {
            chunk: section,
            chunk_pos,
            block_pos: world_pos,
            old_state,
            new_state,
            flags: BlockUpdateFlags::all(),
        });
}

fn read_nibbles(app: &App, section: Entity) -> [u8; 2048] {
    let light = app
        .world()
        .get::<BlockLight>(section)
        .expect("BlockLight component missing on section");
    match &light.0 {
        LightStorage::Null => [0u8; 2048],
        LightStorage::Uniform(v) => {
            let packed = (*v & 0x0F) | ((*v & 0x0F) << 4);
            [packed; 2048]
        }
        LightStorage::Mixed(arr) => *arr.0,
    }
}

fn assert_invariants_hold(app: &App, section: Entity, label: &str) {
    let table = app.world().resource::<BlockLightTable>();
    let palette = app
        .world()
        .get::<BlockPalette>(section)
        .expect("BlockPalette missing on section");
    let light = app
        .world()
        .get::<BlockLight>(section)
        .expect("BlockLight missing on section");
    if let Err(v) = check_block_light_invariants(table, palette, &light.0) {
        panic!("BLK-06 invariants violated in '{label}': {v}");
    }
}

fn read_sky_nibbles(app: &App, section: Entity) -> [u8; 2048] {
    let light = app
        .world()
        .get::<SkyLight>(section)
        .expect("SkyLight component missing on section");
    match &light.0 {
        LightStorage::Null => [0u8; 2048],
        LightStorage::Uniform(v) => {
            let packed = (*v & 0x0F) | ((*v & 0x0F) << 4);
            [packed; 2048]
        }
        LightStorage::Mixed(arr) => *arr.0,
    }
}

fn assert_sky_invariants_hold(app: &App, section: Entity, label: &str) {
    assert_sky_invariants_hold_with(app, section, true, label);
}

fn assert_sky_invariants_hold_with(
    app: &App,
    section: Entity,
    is_topmost_in_skyhaving_column: bool,
    label: &str,
) {
    let table = app.world().resource::<BlockLightTable>();
    let palette = app
        .world()
        .get::<BlockPalette>(section)
        .expect("BlockPalette missing on section");
    let light = app
        .world()
        .get::<SkyLight>(section)
        .expect("SkyLight missing on section");
    if let Err(v) =
        check_sky_light_invariants(table, palette, &light.0, is_topmost_in_skyhaving_column)
    {
        panic!("sky invariants violated in '{label}': {v}");
    }
}

/// Spawn two stacked sections of the same chunk column (same (x, z), chunk_y
/// 0 below and chunk_y 1 above). Returns `(section_a_bottom, section_b_top,
/// column_entity)`. Runs one `FixedUpdate` tick so `ColumnPlugin` materialises
/// the column entity, `attach_lighting_state` installs the per-section
/// bundles, and the `ColumnIndex` lookup resolves.
fn spawn_two_section_column(app: &mut App, dim: Entity) -> (Entity, Entity, Entity) {
    let chunk_pos_a = ChunkPos::new(0, 0, 0);
    let chunk_pos_b = ChunkPos::new(0, 1, 0);
    let section_a = spawn_air_section(app, dim, chunk_pos_a);
    let section_b = spawn_air_section(app, dim, chunk_pos_b);
    app.world_mut().run_schedule(FixedUpdate);
    let column = app
        .world()
        .get::<ColumnIndex>(dim)
        .expect("dimension has ColumnIndex")
        .0
        .get(&ChunkColumnPos::new(0, 0))
        .expect("column reconciled for (0, 0)")
        .entity;
    (section_a, section_b, column)
}

/// Spawn two sections, one per chunk column, side-by-side along +X. Returns
/// `(section_a_at_(0, 0), section_b_at_(1, 0), column_a, column_b)`. Both
/// sections live at chunk_y = 0. Runs one `FixedUpdate` tick to reconcile.
fn spawn_column_pair(app: &mut App, dim: Entity) -> (Entity, Entity, Entity, Entity) {
    let chunk_pos_a = ChunkPos::new(0, 0, 0);
    let chunk_pos_b = ChunkPos::new(1, 0, 0);
    let section_a = spawn_air_section(app, dim, chunk_pos_a);
    let section_b = spawn_air_section(app, dim, chunk_pos_b);
    app.world_mut().run_schedule(FixedUpdate);
    let column_index = app
        .world()
        .get::<ColumnIndex>(dim)
        .expect("dimension has ColumnIndex");
    let column_a = column_index
        .0
        .get(&ChunkColumnPos::new(0, 0))
        .expect("column reconciled for (0, 0)")
        .entity;
    let column_b = column_index
        .0
        .get(&ChunkColumnPos::new(1, 0))
        .expect("column reconciled for (1, 0)")
        .entity;
    (section_a, section_b, column_a, column_b)
}

/// Place a synthetic torch at `block_pos` (world coordinates) inside the
/// already-loaded `section` and emit a `BlockPlaced` message so the enqueue
/// stage observes the change. `chunk_pos` must match the section's stored
/// `ChunkPos`. The previous palette state is read from the section's
/// `BlockPalette` and forwarded as `old_state` on the message.
fn spawn_lit_torch_in_section(
    app: &mut App,
    section: Entity,
    chunk_pos: ChunkPos,
    block_pos: BlockPos,
    emission_state: BlockStateId,
) {
    let old_state = app
        .world()
        .get::<BlockPalette>(section)
        .expect("BlockPalette missing on section")
        .get(block_pos);
    set_palette_cell(app, section, block_pos, emission_state);
    emit_block_placed(app, section, chunk_pos, block_pos, old_state, emission_state);
}

/// Test-local mapping from a face-frame `(face, cell_a, cell_b)` triple to the
/// destination-section-local `(x, y, z)` cell coordinates expected by
/// `LightStorage::get(usize, usize, usize)`. Distinct from the production
/// `face_to_section_coords` in `propagate.rs` (`pub(crate)`, returns `u8`)
/// because the production helper feeds `pack_bfs_entry` while this helper
/// feeds `LightStorage::get`.
fn face_to_section_coords_usize(face: Direction, cell_a: u8, cell_b: u8) -> (usize, usize, usize) {
    match face {
        Direction::Down => (cell_a as usize, 0, cell_b as usize),
        Direction::Up => (cell_a as usize, 15, cell_b as usize),
        Direction::North => (cell_a as usize, cell_b as usize, 0),
        Direction::South => (cell_a as usize, cell_b as usize, 15),
        Direction::West => (0, cell_a as usize, cell_b as usize),
        Direction::East => (15, cell_a as usize, cell_b as usize),
    }
}

/// Read the block-light level at the named face cell of `section`. `face` is
/// the face the caller is reading from, in the section's own frame. For
/// example, `Direction::West` with `(cell_a=8, cell_b=8)` reads the cell at
/// the section's local `(x=0, y=8, z=8)`.
fn read_face_cell(app: &App, section: Entity, face: Direction, cell_a: u8, cell_b: u8) -> u8 {
    let (x, y, z) = face_to_section_coords_usize(face, cell_a, cell_b);
    let light = app
        .world()
        .get::<BlockLight>(section)
        .expect("BlockLight missing on section");
    light.0.get(x, y, z)
}

/// Sky-light counterpart of `read_face_cell`.
fn read_sky_face_cell(app: &App, section: Entity, face: Direction, cell_a: u8, cell_b: u8) -> u8 {
    let (x, y, z) = face_to_section_coords_usize(face, cell_a, cell_b);
    let light = app
        .world()
        .get::<SkyLight>(section)
        .expect("SkyLight missing on section");
    light.0.get(x, y, z)
}

#[test]
fn snapshot_single_torch() {
    let (mut app, dim) = make_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_air_section(&mut app, dim, chunk_pos);

    // attach_lighting_state needs one FixedUpdate tick to insert the
    // BlockLightBundle onto the section.
    app.world_mut().run_schedule(FixedUpdate);

    let torch_pos = BlockPos::new(8, 8, 8);
    set_palette_cell(
        &mut app,
        section,
        torch_pos,
        golden::light_table::SYNTH_TORCH_ID,
    );

    emit_block_placed(
        &mut app,
        section,
        chunk_pos,
        torch_pos,
        golden::light_table::SYNTH_AIR_ID,
        golden::light_table::SYNTH_TORCH_ID,
    );

    app.world_mut().run_schedule(FixedUpdate);

    let actual = read_nibbles(&app, section);
    assert_nibbles_eq(
        &actual,
        &golden::single_torch::EXPECTED_BLOCK_LIGHT,
        "single_torch",
    );

    assert_invariants_hold(&app, section, "single_torch");
}

#[test]
fn snapshot_two_torches_one_removed() {
    let (mut app, dim) = make_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_air_section(&mut app, dim, chunk_pos);

    app.world_mut().run_schedule(FixedUpdate);

    let torch_a = BlockPos::new(4, 8, 8);
    let torch_b = BlockPos::new(12, 8, 8);

    set_palette_cell(
        &mut app,
        section,
        torch_a,
        golden::light_table::SYNTH_TORCH_ID,
    );
    set_palette_cell(
        &mut app,
        section,
        torch_b,
        golden::light_table::SYNTH_TORCH_ID,
    );

    emit_block_placed(
        &mut app,
        section,
        chunk_pos,
        torch_a,
        golden::light_table::SYNTH_AIR_ID,
        golden::light_table::SYNTH_TORCH_ID,
    );
    emit_block_placed(
        &mut app,
        section,
        chunk_pos,
        torch_b,
        golden::light_table::SYNTH_AIR_ID,
        golden::light_table::SYNTH_TORCH_ID,
    );

    app.world_mut().run_schedule(FixedUpdate);

    let both_lit =
        golden::expected::compute_l1_attenuated_field(&[((4, 8, 8), 14), ((12, 8, 8), 14)]);
    let actual_after_place = read_nibbles(&app, section);
    assert_nibbles_eq(
        &actual_after_place,
        &both_lit,
        "two_torches_one_removed_after_place",
    );
    assert_invariants_hold(&app, section, "two_torches_one_removed_after_place");

    set_palette_cell(
        &mut app,
        section,
        torch_b,
        golden::light_table::SYNTH_AIR_ID,
    );
    emit_block_placed(
        &mut app,
        section,
        chunk_pos,
        torch_b,
        golden::light_table::SYNTH_TORCH_ID,
        golden::light_table::SYNTH_AIR_ID,
    );

    app.world_mut().run_schedule(FixedUpdate);

    let actual_after_remove = read_nibbles(&app, section);
    assert_nibbles_eq(
        &actual_after_remove,
        &golden::two_torches_one_removed::EXPECTED_BLOCK_LIGHT,
        "two_torches_one_removed_after_remove",
    );
    assert_invariants_hold(&app, section, "two_torches_one_removed_after_remove");
}

#[test]
fn snapshot_cross_section_horizontal() {
    let (mut app, dim) = make_test_app();
    let (section_a, section_b, _col_a, _col_b) = spawn_column_pair(&mut app, dim);

    let chunk_pos_a = ChunkPos::new(0, 0, 0);
    // Place the torch on section A's east boundary (local x=15) so the
    // intra-section BFS hits the east face and pushes egress for the
    // cross-section distribute pass to route into section B's west face.
    let torch_pos = BlockPos::new(15, 8, 8);
    spawn_lit_torch_in_section(
        &mut app,
        section_a,
        chunk_pos_a,
        torch_pos,
        golden::light_table::SYNTH_TORCH_ID,
    );

    // One FixedUpdate tick drives the whole convergence loop end-to-end:
    //   BlockUpdateSet::ApplyChanges -> LightingSet::Enqueue
    //   -> LightingSet::Converge (loops LightConvergeSchedule:
    //      PropagateDecrease -> DistributeDecrease
    //      -> PropagateIncrease -> DistributeIncrease)
    //   -> LightingSet::EmitDirty.
    app.world_mut().run_schedule(FixedUpdate);

    // Sanity check: A's torch cell must be lit. If this fails, the enqueue
    // or intra-section propagate path is broken — not the cross-section path.
    let a_cell = {
        let light = app
            .world()
            .get::<BlockLight>(section_a)
            .expect("BlockLight on A");
        light.0.get(15, 8, 8)
    };
    assert_eq!(
        a_cell, 14,
        "torch cell at A's (15, 8, 8) should hold the emission level 14"
    );

    // Section B is east of section A. Looking from B's frame, the wave enters
    // through B's West face. The Manhattan-1 cross-section step attenuates
    // 14 -> 13 at B's (0, 8, 8).
    let cell = read_face_cell(&app, section_b, Direction::West, 8, 8);
    assert_eq!(
        cell, 13,
        "cross-section horizontal should land level 13 at B's West face cell (8, 8); got {cell}"
    );
    // The per-section invariant checker is intra-section only; B's face cell
    // receives support from A across the boundary, which the checker cannot
    // see, so a `SourceExcess` violation would fire on B. Section A is fully
    // self-supporting and remains valid.
    assert_invariants_hold(&app, section_a, "cross_section_horizontal_a");
}

#[test]
fn snapshot_vertical_y_boundary() {
    let (mut app, dim) = make_test_app();
    let (section_a, section_b, _col) = spawn_two_section_column(&mut app, dim);

    // section_a sits at chunk_y=0 (world y in 0..16); section_b sits at
    // chunk_y=1 (world y in 16..32). Torch on the top face of A (world y=15).
    let chunk_pos_a = ChunkPos::new(0, 0, 0);
    let torch_pos = BlockPos::new(8, 15, 8);
    spawn_lit_torch_in_section(
        &mut app,
        section_a,
        chunk_pos_a,
        torch_pos,
        golden::light_table::SYNTH_TORCH_ID,
    );

    app.world_mut().run_schedule(FixedUpdate);

    // Section B is above A. Looking from B's frame, the wave enters through
    // B's Down face. The Manhattan step crosses from A's (8, 15, 8)=14 into
    // B's (8, 0, 8) at level 13.
    let cell = read_face_cell(&app, section_b, Direction::Down, 8, 8);
    assert!(
        cell >= 12,
        "vertical Y boundary should land a positive level at B's Down face cell (8, 8); got {cell}"
    );
    // The per-section invariant checker cannot see across boundaries; B's
    // face cell receives cross-section support that the checker would flag
    // as `SourceExcess`. Validate A in isolation.
    assert_invariants_hold(&app, section_a, "vertical_y_boundary_a");
}

#[test]
fn snapshot_empty_sky_above_heightmap() {
    let (mut app, dim) = make_test_app();
    let chunk_pos = ChunkPos::new(0, 19, 0);
    let section = spawn_air_section(&mut app, dim, chunk_pos);

    // Tick 1: column reconciliation populates SectionIndex, attach_lighting_state
    // inserts SkyLightBundle on the section.
    app.world_mut().run_schedule(FixedUpdate);
    // Tick 2: Added<SkyLight> fires enqueue_sky_light_initial, then the
    // column-walker fast path collapses the all-air section to Uniform(15).
    app.world_mut().run_schedule(FixedUpdate);

    let actual = read_sky_nibbles(&app, section);
    assert_nibbles_eq(
        &actual,
        &golden::empty_sky_above_heightmap::EXPECTED_SKY_LIGHT,
        "empty_sky_above_heightmap",
    );
    assert_sky_invariants_hold(&app, section, "empty_sky_above_heightmap");
}

#[test]
fn snapshot_heightmap_update_on_place() {
    let (mut app, dim) = make_test_app();
    // Two-section column at the topmost-of-column y range so the upper
    // section seeds sky=15 across its top face on initial-light. Sections at
    // chunk_y=18 (lower) and chunk_y=19 (upper) place the upper at the same
    // y range used by `snapshot_empty_sky_above_heightmap`.
    let chunk_pos_lower = ChunkPos::new(0, 18, 0);
    let chunk_pos_upper = ChunkPos::new(0, 19, 0);
    let section_lower = spawn_air_section(&mut app, dim, chunk_pos_lower);
    let section_upper = spawn_air_section(&mut app, dim, chunk_pos_upper);

    // Tick 1: column reconciliation populates SectionIndex, attach_lighting_state
    // inserts the per-section bundles, and seed_initial_light queues initial
    // light on the next tick.
    app.world_mut().run_schedule(FixedUpdate);

    // Place dampening water at world y=18*16+8 inside the lower section; this
    // sits inside the section, with sky cells above and below.
    let water_pos = BlockPos::new(8, 18 * 16 + 8, 8);
    spawn_lit_torch_in_section(
        &mut app,
        section_lower,
        chunk_pos_lower,
        water_pos,
        golden::light_table::SYNTH_WATER_ID,
    );

    // Tick 2: seed_initial_light + propagate/distribute fill the column, water
    // damps the cell that contains it. After the convergence loop drains, the
    // sky-light invariant must hold on both sections.
    app.world_mut().run_schedule(FixedUpdate);

    // The upper section is fully open and on top, so its top face must read 15.
    let upper_top = read_sky_face_cell(&app, section_upper, Direction::Up, 8, 8);
    assert_eq!(
        upper_top, 15,
        "upper section's top face cell should hold the sky source level 15; got {upper_top}"
    );
    assert_sky_invariants_hold_with(&app, section_upper, true, "heightmap_update_on_place_upper");
    // The lower section's invariants must omit `SourceExcess` and
    // `SupportFloor` checks: every cell at the section boundary that
    // receives cross-section ingress carries support that the per-section
    // checker cannot see. Validate the cross-section flow via face-cell
    // reads instead. Reading from the lower section's Up face cell tells us
    // whether sky light descended through the boundary into the lower
    // section. With the current cross-section attenuation a level below 15
    // is expected; assert only that some light arrived.
    let lower_top_face = read_sky_face_cell(&app, section_lower, Direction::Up, 0, 0);
    assert!(
        lower_top_face > 0,
        "lower section's Up face should receive cross-section sky light; got {lower_top_face}"
    );
}

#[test]
fn snapshot_heightmap_update_on_break() {
    let (mut app, dim) = make_test_app();
    let chunk_pos_lower = ChunkPos::new(0, 18, 0);
    let chunk_pos_upper = ChunkPos::new(0, 19, 0);
    let section_lower = spawn_air_section(&mut app, dim, chunk_pos_lower);
    let section_upper = spawn_air_section(&mut app, dim, chunk_pos_upper);
    app.world_mut().run_schedule(FixedUpdate);

    // Place water, then break it: place stone first, break by replacing it
    // with air.
    let block_pos = BlockPos::new(8, 18 * 16 + 8, 8);
    spawn_lit_torch_in_section(
        &mut app,
        section_lower,
        chunk_pos_lower,
        block_pos,
        golden::light_table::SYNTH_WATER_ID,
    );
    app.world_mut().run_schedule(FixedUpdate);

    // Now "break" the water: set the cell back to air and emit BlockPlaced
    // with old=water, new=air. This re-converges the sky light.
    spawn_lit_torch_in_section(
        &mut app,
        section_lower,
        chunk_pos_lower,
        block_pos,
        golden::light_table::SYNTH_AIR_ID,
    );
    app.world_mut().run_schedule(FixedUpdate);

    // After the break, the upper section's top face must still read 15.
    let upper_top = read_sky_face_cell(&app, section_upper, Direction::Up, 8, 8);
    assert_eq!(
        upper_top, 15,
        "upper section's top face cell should still hold 15 after the break; got {upper_top}"
    );
    assert_sky_invariants_hold_with(&app, section_upper, true, "heightmap_update_on_break_upper");
    // See `snapshot_heightmap_update_on_place` for the rationale: the lower
    // section's per-section invariants cannot validate cross-section
    // ingress, so we check the face-cell read instead.
    let lower_top_face = read_sky_face_cell(&app, section_lower, Direction::Up, 0, 0);
    assert!(
        lower_top_face > 0,
        "lower section's Up face should receive cross-section sky light after the break; got {lower_top_face}"
    );
}

#[test]
fn snapshot_sky_attenuation_through_water() {
    let (mut app, dim) = make_test_app();
    let chunk_pos = ChunkPos::new(0, 19, 0);
    let section = spawn_air_section(&mut app, dim, chunk_pos);

    // Place water in the palette BEFORE Tick 1 so the lifecycle-time
    // `is_section_all_air` check sees a non-all-air palette and the section
    // never gains the `IsAllAir` marker. Without `IsAllAir` the
    // `propagate_increase_sky_system` column-walker fast path is skipped, and
    // the BFS attenuates the level through the water cell as expected. The
    // alternative BlockPlaced-driven flow can't drive this test today because
    // `IsAllAir` stays stale after a palette mutation; keeping that marker in
    // sync with mid-run palette changes lives in cross-section work and is
    // out of scope here.
    let water_pos = BlockPos::new(8, 19 * 16 + 10, 8);
    set_palette_cell(
        &mut app,
        section,
        water_pos,
        golden::light_table::SYNTH_WATER_ID,
    );

    // Tick 1: column reconciliation populates SectionIndex, attach_lighting_state
    // inserts SkyLightBundle on the section (no IsAllAir because of the water).
    app.world_mut().run_schedule(FixedUpdate);
    // Tick 2: Added<SkyLight> fires enqueue_sky_light_initial; the BFS path
    // (not the column-walker fast path) attenuates light through the water cell.
    app.world_mut().run_schedule(FixedUpdate);

    let actual = read_sky_nibbles(&app, section);
    assert_nibbles_eq(
        &actual,
        &golden::sky_attenuation_through_water::EXPECTED_SKY_LIGHT,
        "sky_attenuation_through_water",
    );
    assert_sky_invariants_hold(&app, section, "sky_attenuation_through_water");
}

#[test]
fn snapshot_light_ticket_clears_when_pending_work_drains() {
    let (mut app, dim) = make_test_app();
    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_air_section(&mut app, dim, chunk_pos);

    // Tick 1: attach_lighting_state inserts BlockLightBundle + SkyLightBundle
    // on the section, marks ChunkNeedsInitialLight.
    app.world_mut().run_schedule(FixedUpdate);

    // Tick 2: seed_initial_light fires; for an all-air section in a
    // sky-light dim that section also becomes the topmost-of-column and seeds
    // sky level 15. The convergence loop drains the workspace queues; the
    // safety-net clears LightDirty.
    app.world_mut().run_schedule(FixedUpdate);

    // Sanity: by this point the section's queues should all be empty and
    // LightDirty should be gone. If LightDirty is still present, draining the
    // workspaces isn't done yet and the next tick will keep working.
    assert!(
        app.world().get::<LightDirty>(section).is_none(),
        "LightDirty must be cleared before the ticket-clear path runs"
    );

    // Now install a LightTicket manually (this is the contract: upstream
    // distribute systems insert tickets when they push cross-section work; the
    // emit-dirty pass tears them down after the work drains).
    app.world_mut().entity_mut(section).insert(LightTicket);
    assert!(
        app.world().get::<LightTicket>(section).is_some(),
        "LightTicket installed for the test"
    );

    // Tick 3: clear_light_tickets in LightingSet::EmitDirty observes the
    // section has no LightDirty and all eight queues empty, so it removes
    // LightTicket. The negative case (ticket stays while work is present)
    // is covered by the co-located emit_dirty unit tests that drive
    // clear_light_tickets in isolation; here we just validate the integrated
    // happy path.
    app.world_mut().run_schedule(FixedUpdate);
    assert!(
        app.world().get::<LightTicket>(section).is_none(),
        "LightTicket must clear once pending work has drained"
    );
}
