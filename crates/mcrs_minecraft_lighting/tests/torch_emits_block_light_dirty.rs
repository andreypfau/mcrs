// INT-05: BlockSetRequest for an emitter block produces exactly one
// `BlockLightDirty` message in the same `FixedUpdate` tick.
//
// Boots `ColumnPlugin + BlockUpdatePlugin + LightingPlugin` (no network /
// login / configuration plugins — those would require a live TCP session
// without adding INT-05 coverage). The chain under test is
// `BlockSetRequest → apply_set_block_request → BlockPlaced →
// enqueue_block_light_on_block_placed → propagate → emit_block_light_dirty
// → BlockLightDirty`, all within one `FixedUpdate` run.
//
// The stub `BlockLightTable` is extended to a third state (BlockStateId(2))
// with emission = 14 (vanilla torch level) so the placement actually seeds
// a block-light source and the dirty marker fires.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;
use mcrs_core::AppState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkIndex, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    ColumnPos, ColumnPosComponent, ColumnPlugin, InColumn,
};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::{BlockSetRequest, BlockUpdatePlugin};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::codec::BlockLightDirty;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::telemetry::TELEMETRY_TEST_LOCK;
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_protocol::BlockStateId;

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;

fn make_test_app_with_block_update(sky: bool) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(BlockUpdatePlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_block_light_table_with_torch());
    let dim_entity = spawn_test_dimension(&mut app, sky);
    (app, dim_entity)
}

fn make_stub_block_light_table_with_torch() -> BlockLightTable {
    let state_count = 3usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    // State 0: air-equivalent. Sky propagates straight down through it.
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    // State 1: solid opaque block.
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] =
        flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    // State 2: torch-stub emitter. Level 14 matches vanilla torch.
    emission[2] = 14;
    dampening[2] = 0;
    flags[2] = flag_bits::IS_NOT_AIR;
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

fn spawn_test_section(
    app: &mut App,
    dim: Entity,
    chunk_pos: ChunkPos,
    palette: BlockPalette,
) -> Entity {
    let section = app
        .world_mut()
        .spawn((
            InDimension(dim),
            chunk_pos,
            ChunkEntities::default(),
            Chunk,
            ChunkLoaded,
            palette,
        ))
        .id();
    // Manually register the section in the dimension's `ChunkIndex`. The
    // production path runs this via the ticket / spawn_chunks pipeline; tests
    // bypass that surface and spawn sections directly, so the index must be
    // primed by hand. `apply_set_block_request` looks the section up through
    // `ChunkIndex::get(chunk_pos)`, so without this mapping the request is
    // silently dropped.
    app.world_mut()
        .entity_mut(dim)
        .get_mut::<ChunkIndex>()
        .expect("dimension must have ChunkIndex from DimensionBundle")
        .insert(chunk_pos, section);
    section
}

fn air_palette() -> BlockPalette {
    let mut p = BlockPalette::default();
    p.fill(BlockStateId(0));
    p
}

#[test]
fn torch_placement_emits_exactly_one_block_light_dirty_message() {
    let _lock = TELEMETRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let (mut app, dim) = make_test_app_with_block_update(true);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);

    let chunk_pos = ChunkPos::new(0, 0, 0);
    let section = spawn_test_section(&mut app, dim, chunk_pos, air_palette());

    // Quiesce the cold-boot warm-up before placing the torch so the resulting
    // BlockLightDirty count is attributable to the placement alone.
    for _ in 0..3 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    app.world_mut()
        .resource_mut::<Messages<BlockLightDirty>>()
        .clear();

    let column_pos = {
        let world = app.world();
        let in_col = world
            .get::<InColumn>(section)
            .expect("section must have InColumn back-link after warm-up");
        let col_pos_component = world
            .get::<ColumnPosComponent>(in_col.0)
            .expect("column entity must carry ColumnPosComponent");
        col_pos_component.0
    };
    // `ChunkPos -> ColumnPos` strips the y axis; the column at chunk_pos
    // (0, 0, 0) lives at column pos (0, 0).
    assert_eq!(column_pos, ColumnPos::new(0, 0));

    // Sanity check: warm-up must have populated the dimension's ChunkIndex
    // mapping and added the ChunkNetworkSyncBlockChangesSet so that
    // `apply_set_block_request` can locate the section and mutate its
    // BlockPalette in the same tick the request is read.
    {
        let world = app.world();
        let idx = world
            .get::<ChunkIndex>(dim)
            .expect("dimension must have ChunkIndex");
        assert_eq!(
            idx.get(chunk_pos),
            Some(section),
            "ChunkIndex must point chunk_pos -> section before the torch placement"
        );
        assert!(
            world
                .get::<mcrs_minecraft_block::block_update::ChunkNetworkSyncBlockChangesSet>(section)
                .is_some(),
            "BlockUpdatePlugin::add_changes_set must have attached the changes set during warm-up"
        );
    }

    // Place a torch at the section center. `BlockPos::new(8, 8, 8) >> 4` lands
    // in `ChunkPos::new(0, 0, 0)`, matching the spawned section. ALL_IMMEDIATE
    // sets NEIGHBORS | CLIENTS | IMMEDIATE so the apply system actually
    // executes the write.
    app.world_mut()
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension: dim,
            pos: BlockPos::new(8, 8, 8),
            new_state: BlockStateId(2),
            flags: BlockUpdateFlags::ALL_IMMEDIATE,
            recursion_left: 512,
        });

    app.world_mut().run_schedule(FixedUpdate);

    // Confirm `apply_set_block_request` wrote the torch state into the
    // palette; without this the rest of the chain is moot and the
    // BlockLightDirty assertion would mis-diagnose the failure.
    let world = app.world();
    let palette_state = world
        .get::<BlockPalette>(section)
        .map(|p| p.get(BlockPos::new(8, 8, 8)))
        .expect("section must still have BlockPalette");
    assert_eq!(
        palette_state, BlockStateId(2),
        "apply_set_block_request must have replaced the cell with the torch state"
    );

    let msgs = world.resource::<Messages<BlockLightDirty>>();
    // `iter_current_update_messages` only sees the latest-tick buffer; the
    // emit-dirty pass runs during this same FixedUpdate so the message lives
    // in that buffer.
    let collected: Vec<_> = msgs.iter_current_update_messages().collect();
    assert_eq!(
        collected.len(),
        1,
        "exactly one BlockLightDirty message expected for one torch in one section, got: {:?}",
        collected
            .iter()
            .map(|m| (m.section, m.column_pos, m.chunk_y))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        collected[0].section, section,
        "BlockLightDirty must target the section receiving the torch placement"
    );
    assert_eq!(
        collected[0].column_pos, column_pos,
        "BlockLightDirty must carry the parent column's ColumnPos"
    );
}
