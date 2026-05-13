#![cfg(feature = "test-bench")]

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use mcrs_core::AppState;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::ColumnPlugin;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_lighting::components::{BlockLight, SkyLight};
use mcrs_lighting::invariants::{check_block_light_invariants, check_sky_light_invariants};
use mcrs_lighting::storage::LightStorage;
use mcrs_lighting::stub::{block_light_nibbles, sky_light_nibbles};
use mcrs_lighting::table::BlockLightTable;
use mcrs_lighting::test_bench::{assert_nibbles_eq, from_input};
use mcrs_lighting::LightingPlugin;
use mcrs_minecraft::world::block::BlockUpdateFlags;
use mcrs_minecraft::world::block_update::BlockPlaced;
use mcrs_minecraft::world::palette::BlockPalette;
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
    let table = app.world().resource::<BlockLightTable>();
    let palette = app
        .world()
        .get::<BlockPalette>(section)
        .expect("BlockPalette missing on section");
    let light = app
        .world()
        .get::<SkyLight>(section)
        .expect("SkyLight missing on section");
    if let Err(v) = check_sky_light_invariants(table, palette, &light.0, true) {
        panic!("sky invariants violated in '{label}': {v}");
    }
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
#[ignore = "cross-section distribute not yet implemented"]
fn snapshot_cross_section_horizontal() {
    let palette = from_input(golden::cross_section_horizontal::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(
        &actual,
        &golden::cross_section_horizontal::EXPECTED_BLOCK_LIGHT,
        "cross_section_horizontal",
    );
}

#[test]
#[ignore = "cross-section distribute not yet implemented"]
fn snapshot_vertical_y_boundary() {
    let palette = from_input(golden::vertical_y_boundary::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(
        &actual,
        &golden::vertical_y_boundary::EXPECTED_BLOCK_LIGHT,
        "vertical_y_boundary",
    );
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
#[ignore = "cross-section sky propagation not yet implemented"]
fn snapshot_heightmap_update_on_place() {
    let palette = from_input(golden::heightmap_update_on_place::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(
        &actual,
        &golden::heightmap_update_on_place::EXPECTED_SKY_LIGHT,
        "heightmap_update_on_place",
    );
}

#[test]
#[ignore = "cross-section sky propagation not yet implemented"]
fn snapshot_heightmap_update_on_break() {
    let palette = from_input(golden::heightmap_update_on_break::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(
        &actual,
        &golden::heightmap_update_on_break::EXPECTED_SKY_LIGHT,
        "heightmap_update_on_break",
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
