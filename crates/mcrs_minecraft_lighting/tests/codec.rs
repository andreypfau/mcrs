// Integration tests for the lighting wire codec.
//
// `codec_emits_column_light_update_for_dirty_chunks` and the per-column
// merge test inject synthetic `BlockLightDirty` / `SkyLightDirty` messages
// directly into the message buffer and exercise the consumer side of the
// codec. `emit_dirty_writes_block_light_dirty_for_modified_chunk` drives
// the producer wiring via the `LightDirty` marker and confirms the emit-pass
// systems fan that marker out to the per-layer message stream.

use bevy_app::{App, FixedPostUpdate, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::AppExtStates;
use bevy_state::app::StatesPlugin;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::column::{
    Column, ColumnPos, ColumnPosComponent, ColumnPlugin, InColumn,
    ColumnChunks,
};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_lighting::components::{BlockLight, LightDirty, SkyLight};
use mcrs_minecraft_lighting::nibble::NibbleArray;
use mcrs_minecraft_lighting::storage::LightStorage;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::{BlockLightDirty, ColumnLightUpdate, LightingPlugin, SkyLightDirty};

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;
const MIN_CHUNK_Y: i32 = TEST_DIM_MIN_Y / 16; // -4
const CHUNK_COUNT: usize = (TEST_DIM_HEIGHT / 16) as usize; // 24

fn make_codec_test_app() -> (App, Entity) {
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
            dimension_id: DimensionId::new("test:codec"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(entity).insert(HasSkyLight);
    entity
}

struct ColumnHandles {
    column: Entity,
    column_pos: ColumnPos,
    chunks: Vec<Entity>,
}

fn spawn_test_column(app: &mut App, dim: Entity, column_pos: ColumnPos) -> ColumnHandles {
    let mut chunk_index = ColumnChunks::new(MIN_CHUNK_Y, CHUNK_COUNT);
    let mut chunk_entities: Vec<Entity> = Vec::with_capacity(CHUNK_COUNT);
    let column = app
        .world_mut()
        .spawn((
            ColumnPosComponent(column_pos),
            InDimension(dim),
            Column,
        ))
        .id();
    for i in 0..CHUNK_COUNT {
        let chunk_y = MIN_CHUNK_Y + i as i32;
        let chunk_entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Null),
                SkyLight(LightStorage::Null),
                InColumn(column),
            ))
            .id();
        chunk_index.set_loaded(chunk_y, chunk_entity);
        chunk_entities.push(chunk_entity);
    }
    app.world_mut().entity_mut(column).insert(chunk_index);
    ColumnHandles {
        column,
        column_pos,
        chunks: chunk_entities,
    }
}

fn wire_bit_for_chunk_y(chunk_y: i32) -> usize {
    (chunk_y - MIN_CHUNK_Y + 1) as usize
}

fn bit_is_set(mask: &[u64], bit_idx: usize) -> bool {
    let word_idx = bit_idx / 64;
    if word_idx >= mask.len() {
        return false;
    }
    (mask[word_idx] >> (bit_idx % 64)) & 1 == 1
}

fn popcount(mask: &[u64]) -> u32 {
    mask.iter().map(|w| w.count_ones()).sum()
}

fn inject_block_dirty(app: &mut App, chunk: Entity, column_pos: ColumnPos, chunk_y: i32) {
    app.world_mut()
        .resource_mut::<Messages<BlockLightDirty>>()
        .write(BlockLightDirty {
            chunk,
            column_pos,
            chunk_y,
        });
}

fn inject_sky_dirty(app: &mut App, chunk: Entity, column_pos: ColumnPos, chunk_y: i32) {
    app.world_mut()
        .resource_mut::<Messages<SkyLightDirty>>()
        .write(SkyLightDirty {
            chunk,
            column_pos,
            chunk_y,
        });
}

fn drain_column_light_updates(app: &mut App) -> Vec<ColumnLightUpdate> {
    app.world_mut()
        .resource_mut::<Messages<ColumnLightUpdate>>()
        .drain()
        .collect()
}

fn drain_block_light_dirty(app: &mut App) -> Vec<BlockLightDirty> {
    app.world_mut()
        .resource_mut::<Messages<BlockLightDirty>>()
        .drain()
        .collect()
}

#[test]
fn codec_emits_column_light_update_for_dirty_chunks() {
    let (mut app, dim) = make_codec_test_app();
    let handles = spawn_test_column(&mut app, dim, ColumnPos::new(3, -7));

    let chunk_y: i32 = 5;
    let target_chunk = handles.chunks[(chunk_y - MIN_CHUNK_Y) as usize];

    // Give the dirty chunk a non-default block-light storage so the codec
    // emits a populated mask bit (a Null storage would route to the empty
    // mask instead).
    let mut nibble = NibbleArray::zeros();
    nibble.set(2, 4, 6, 0xA);
    app.world_mut()
        .entity_mut(target_chunk)
        .insert(BlockLight(LightStorage::Mixed(Box::new(nibble))));

    inject_block_dirty(&mut app, target_chunk, handles.column_pos, chunk_y);

    app.world_mut().run_schedule(FixedPostUpdate);

    let updates = drain_column_light_updates(&mut app);
    assert_eq!(updates.len(), 1, "exactly one ColumnLightUpdate emitted");
    let update = &updates[0];
    assert_eq!(update.column, handles.column);
    assert_eq!(update.column_pos, handles.column_pos);
    assert_eq!(update.dim, dim);

    let block_mask: &[u64] = &update.light_data.block_light_mask;
    assert_eq!(
        popcount(block_mask),
        1,
        "exactly one block-light mask bit set; got {:?}",
        block_mask
    );
    let expected_bit = wire_bit_for_chunk_y(chunk_y);
    assert!(
        bit_is_set(block_mask, expected_bit),
        "block-light bit {} (for chunk_y={}) must be set",
        expected_bit,
        chunk_y
    );
    assert_eq!(
        update.light_data.block_light_arrays.len(),
        1,
        "one block-light payload appended for the dirty chunk"
    );
    // No sky-light bits should be set because only BlockLightDirty was
    // injected.
    let sky_mask: &[u64] = &update.light_data.sky_light_mask;
    assert_eq!(popcount(sky_mask), 0, "sky mask must stay clear");
}

#[test]
fn codec_merges_block_and_sky_dirty_into_one_packet() {
    let (mut app, dim) = make_codec_test_app();
    let handles = spawn_test_column(&mut app, dim, ColumnPos::new(0, 0));

    let chunk_y: i32 = 2;
    let target_chunk = handles.chunks[(chunk_y - MIN_CHUNK_Y) as usize];

    // Both layers get a non-default storage so the codec emits populated
    // mask bits on each side rather than empty-mask placeholders.
    let mut block_nibble = NibbleArray::zeros();
    block_nibble.set(0, 0, 0, 0x7);
    let mut sky_nibble = NibbleArray::zeros();
    sky_nibble.set(0, 0, 0, 0xF);
    app.world_mut().entity_mut(target_chunk).insert((
        BlockLight(LightStorage::Mixed(Box::new(block_nibble))),
        SkyLight(LightStorage::Mixed(Box::new(sky_nibble))),
    ));

    inject_block_dirty(&mut app, target_chunk, handles.column_pos, chunk_y);
    inject_sky_dirty(&mut app, target_chunk, handles.column_pos, chunk_y);

    app.world_mut().run_schedule(FixedPostUpdate);

    let updates = drain_column_light_updates(&mut app);
    assert_eq!(
        updates.len(),
        1,
        "block + sky dirty for the same column collapse into one packet"
    );
    let update = &updates[0];
    let expected_bit = wire_bit_for_chunk_y(chunk_y);
    assert_eq!(popcount(&update.light_data.block_light_mask), 1);
    assert_eq!(popcount(&update.light_data.sky_light_mask), 1);
    assert!(bit_is_set(&update.light_data.block_light_mask, expected_bit));
    assert!(bit_is_set(&update.light_data.sky_light_mask, expected_bit));
    assert_eq!(update.light_data.block_light_arrays.len(), 1);
    assert_eq!(update.light_data.sky_light_arrays.len(), 1);
}

#[test]
fn codec_emits_no_message_when_no_dirty_inputs() {
    let (mut app, dim) = make_codec_test_app();
    let _handles = spawn_test_column(&mut app, dim, ColumnPos::new(4, 4));

    app.world_mut().run_schedule(FixedPostUpdate);

    let updates = drain_column_light_updates(&mut app);
    assert!(
        updates.is_empty(),
        "codec must not emit messages when no dirty inputs arrived; got {:?}",
        updates.len()
    );
}

#[test]
fn emit_dirty_writes_block_light_dirty_for_modified_chunk() {
    let (mut app, dim) = make_codec_test_app();
    let handles = spawn_test_column(&mut app, dim, ColumnPos::new(-1, 1));

    let chunk_y: i32 = 3;
    let target_chunk = handles.chunks[(chunk_y - MIN_CHUNK_Y) as usize];

    // Simulate a propagation pass having touched this chunk: replace the
    // default Null storage with a Mixed buffer and insert the LightDirty
    // marker the propagate systems would have left behind.
    let mut nibble = NibbleArray::zeros();
    nibble.set(1, 1, 1, 0x5);
    app.world_mut().entity_mut(target_chunk).insert((
        BlockLight(LightStorage::Mixed(Box::new(nibble))),
        LightDirty,
    ));

    app.world_mut().run_schedule(FixedUpdate);

    let block_dirty = drain_block_light_dirty(&mut app);
    assert!(
        !block_dirty.is_empty(),
        "emit_block_light_dirty must produce at least one message for the LightDirty chunk"
    );
    let from_target: Vec<_> = block_dirty
        .iter()
        .filter(|msg| msg.chunk == target_chunk)
        .collect();
    assert_eq!(
        from_target.len(),
        1,
        "exactly one BlockLightDirty for the target chunk; got {} of {}",
        from_target.len(),
        block_dirty.len()
    );
    let msg = from_target[0];
    assert_eq!(msg.column_pos, handles.column_pos);
    assert_eq!(msg.chunk_y, chunk_y);
}
