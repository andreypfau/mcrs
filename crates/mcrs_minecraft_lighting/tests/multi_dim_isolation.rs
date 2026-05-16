// cross-dim isolation regression guard: a torch placement in one Dimension produces zero
// observable effect on any chunk of another Dimension. Three orthogonal
// assertions on the same invariant:
//
//   1. Byte-equality on the unaffected dim's per-chunk storage snapshot —
//      every `BlockLight` and `SkyLight` storage component on every Dim B
//      chunk is byte-identical pre/post the Dim A torch placement.
//
//   2. Message-leak filter — every `BlockLightDirty` and `SkyLightDirty`
//      message emitted in the torch-placement tick references a chunk whose
//      parent `InDimension` is Dim A. Zero messages reference any Dim B
//      chunk.
//
//   3. Counter-delta — `LIGHT_CROSS_DIM_VIOLATIONS_TOTAL` shows no increment
//      across the entire scenario (the `debug_assert!(neighbor_dim ==
//      source_dim)` site in `distribute.rs` never fires).
//
// The dimensions are spawned asymmetrically: Dim A has `HasSkyLight`, Dim B
// does not. The asymmetry stresses the cross-dim isolation invariant against
// both the sky-light and block-light wavefront paths.
//
// Scaffolding is duplicated inline from `lifecycle_integration.rs:41-121`
// following the established sibling-test pattern. The torch trigger writes
// `BlockPlaced` directly against the target chunk entity (the same proven
// pattern as `golden_snapshots.rs:76-94`). Going through `BlockSetRequest`
// would also require manually populating each dimension's `ChunkIndex` with
// the spawned chunk entities — work that `ColumnPlugin` does not perform
// in this test-app shape — and `BlockPlaced` is exactly what the lighting
// engine observes (`crates/mcrs_minecraft_lighting/src/enqueue.rs:39-115`).

use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::entity::ChunkEntities;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{Chunk, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::ColumnPlugin;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::codec::{BlockLightDirty, SkyLightDirty};
use mcrs_minecraft_lighting::components::{BlockLight, SkyLight};
use mcrs_minecraft_lighting::storage::LightStorage;
use mcrs_minecraft_lighting::table::{flag_bits, BlockLightTable};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_protocol::BlockStateId;

const TEST_DIM_HEIGHT: u32 = 384;
const TEST_DIM_MIN_Y: i32 = -64;

const TORCH_STATE: BlockStateId = BlockStateId(2);

fn make_stub_block_light_table_with_torch() -> BlockLightTable {
    let state_count = 3usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    // State 0: air-equivalent (no emission, no dampening, sky propagates down).
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    // State 1: solid opaque (full dampening).
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] =
        flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    // State 2: torch (emitter, no dampening, not air).
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
                "test:par05-sky"
            } else {
                "test:par05-skyless"
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

// Normalises any `LightStorage` variant to a flat 2048-byte representation so
// pre/post snapshots can be compared with `assert_eq!`. The production types
// (`BlockLight`, `SkyLight`, `LightStorage`, `NibbleArray`) do not derive
// `PartialEq`; converting to bytes is the cheapest way to assert structural
// equality without changing production derives. Mirrors the helper at
// `tests/golden_snapshots.rs:96-109`.
fn storage_bytes(storage: &LightStorage) -> [u8; 2048] {
    match storage {
        LightStorage::Null => [0u8; 2048],
        LightStorage::Uniform(v) => {
            let packed = (*v & 0x0F) | ((*v & 0x0F) << 4);
            [packed; 2048]
        }
        LightStorage::Mixed(arr) => *arr.0,
    }
}

// `SkyLight` is gated on `HasSkyLight` per the lifecycle attachment system;
// skyless-dim chunks never carry it. `Option<[u8; 2048]>` so the equality
// check survives the absence of the component.
fn chunk_light_bytes(world: &World, chunk: Entity) -> ([u8; 2048], Option<[u8; 2048]>) {
    let bl = world
        .get::<BlockLight>(chunk)
        .map(|c| storage_bytes(&c.0))
        .expect("every chunk must carry a BlockLight component after warm-up");
    let sl = world.get::<SkyLight>(chunk).map(|c| storage_bytes(&c.0));
    (bl, sl)
}

#[test]
fn torch_in_dim_a_leaves_dim_b_byte_identical_and_no_cross_dim_violation() {
    let _lock = TELEMETRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let before = snapshot();

    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin);
    app.insert_resource(make_stub_block_light_table_with_torch());
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);

    let dim_a = spawn_test_dimension(&mut app, true);
    let dim_b = spawn_test_dimension(&mut app, false);

    let sec_a0 = spawn_test_chunk(&mut app, dim_a, ChunkPos::new(0, 0, 0), air_palette());
    let _sec_a1 = spawn_test_chunk(&mut app, dim_a, ChunkPos::new(1, 0, 0), air_palette());
    let sec_b0 = spawn_test_chunk(&mut app, dim_b, ChunkPos::new(0, 0, 0), air_palette());
    let sec_b1 = spawn_test_chunk(&mut app, dim_b, ChunkPos::new(1, 0, 0), air_palette());

    // Drive initial convergence to quiescence. Four ticks: the
    // `Added<ChunkLoaded>` first-observation path can settle in three on a
    // one-chunk dim, but the two-chunk-per-dim layout gives the cross-
    // chunk distribute pass extra work and the warm-up may need a fourth
    // tick to fully drain.
    for _ in 0..4 {
        app.world_mut().run_schedule(FixedUpdate);
    }

    // Clear any residual dirty-light messages so the post-tick assertion sees
    // only the torch-induced output.
    app.world_mut()
        .resource_mut::<Messages<BlockLightDirty>>()
        .clear();
    app.world_mut()
        .resource_mut::<Messages<SkyLightDirty>>()
        .clear();

    // Snapshot the pre-tick Dim B storage. Skyless-dim chunks do not carry
    // `SkyLight`, so the helper returns `None` for the sky byte block.
    let dim_b_chunks = [sec_b0, sec_b1];
    let pre_b: Vec<([u8; 2048], Option<[u8; 2048]>)> = {
        let world = app.world();
        dim_b_chunks
            .iter()
            .map(|&e| chunk_light_bytes(world, e))
            .collect()
    };

    // Snapshot Dim A's target chunk block-light bytes as a sanity guard so
    // the byte-equality result below is meaningful — if the torch placement
    // somehow produced no observable change in Dim A either, the cross-dim isolation
    // assertion would trivially hold and mask the regression it is meant to
    // catch.
    let pre_a_bytes = chunk_light_bytes(app.world(), sec_a0).0;

    // Place a torch in Dim A's chunk (0, 0, 0) by mutating the palette and
    // writing a `BlockPlaced` message directly against the chunk entity.
    // This is the proven pattern from `tests/golden_snapshots.rs:76-94`.
    let torch_pos = BlockPos::new(8, 8, 8);
    app.world_mut()
        .get_mut::<BlockPalette>(sec_a0)
        .expect("Dim A chunk must have BlockPalette")
        .set(torch_pos, TORCH_STATE);
    app.world_mut()
        .resource_mut::<Messages<BlockPlaced>>()
        .write(BlockPlaced {
            chunk: sec_a0,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos: torch_pos,
            old_state: BlockStateId(0),
            new_state: TORCH_STATE,
            flags: BlockUpdateFlags::all(),
        });

    app.world_mut().run_schedule(FixedUpdate);

    // Cross-dim leak assertion: every emitted dirty-light message references
    // a chunk whose parent dim is Dim A. The current production wire only
    // fires when `LightDirty` survives the in-tick convergence (e.g., bounded-
    // loop cap, cross-chunk back-pressure); under simple-load scenarios the
    // chain may emit zero messages, in which case the loop trivially passes —
    // the byte-equality and counter-delta assertions below are the load-
    // bearing cross-dim isolation guards.
    {
        let world = app.world();
        let block_dirty = world.resource::<Messages<BlockLightDirty>>();
        for msg in block_dirty.iter_current_update_messages() {
            let in_dim = world
                .get::<InDimension>(msg.chunk)
                .expect("BlockLightDirty.chunk must carry InDimension")
                .0;
            assert_eq!(
                in_dim, dim_a,
                "BlockLightDirty referenced a chunk outside Dim A (entity = {:?})",
                msg.chunk
            );
        }

        let sky_dirty = world.resource::<Messages<SkyLightDirty>>();
        for msg in sky_dirty.iter_current_update_messages() {
            let in_dim = world
                .get::<InDimension>(msg.chunk)
                .expect("SkyLightDirty.chunk must carry InDimension")
                .0;
            assert_eq!(
                in_dim, dim_a,
                "SkyLightDirty referenced a chunk outside Dim A (entity = {:?})",
                msg.chunk
            );
        }
    }

    // Byte-equality assertion: every chunk in Dim B has identical block-
    // and sky-light storage pre/post the Dim A torch tick.
    let post_b: Vec<([u8; 2048], Option<[u8; 2048]>)> = {
        let world = app.world();
        dim_b_chunks
            .iter()
            .map(|&e| chunk_light_bytes(world, e))
            .collect()
    };
    for (idx, (pre, post)) in pre_b.iter().zip(post_b.iter()).enumerate() {
        assert_eq!(
            pre.0, post.0,
            "Dim B chunk {idx} BlockLight bytes drifted across the Dim A torch tick"
        );
        assert_eq!(
            pre.1, post.1,
            "Dim B chunk {idx} SkyLight bytes drifted across the Dim A torch tick"
        );
    }

    // Sanity guard: the torch placement must have changed Dim A's storage so
    // the Dim B equality result is meaningful. If propagation never reached
    // the chunk, the test would be a tautology against a no-op scenario.
    let post_a_bytes = chunk_light_bytes(app.world(), sec_a0).0;
    assert_ne!(
        pre_a_bytes, post_a_bytes,
        "Dim A target chunk BlockLight must change after a torch placement \
         — without this, the Dim B equality assertion is a tautology"
    );

    // Counter-delta assertion: the cross-dim guard at `distribute.rs:401, 481`
    // never fired during the scenario.
    let after = snapshot();
    assert_eq!(
        after.cross_dim, before.cross_dim,
        "LIGHT_CROSS_DIM_VIOLATIONS_TOTAL must not increment for a within-Dim torch placement"
    );
}
