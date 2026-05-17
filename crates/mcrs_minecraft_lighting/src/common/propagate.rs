//! Channel-shared propagation helpers.
//!
//! The per-channel propagate systems live under
//! `crate::{block_light,sky_light}::propagate`. Each is a thin Bevy wrapper
//! that drains its chunk's `*Incoming` buffer into the BFS workspace
//! `increase_queue` via `drain_incoming_into_queue`, then dispatches to
//! `bfs::propagate_{increase,decrease}` (or the sky-side mirrors).
//!
//! Drain-inbox prelude: every per-chunk iteration starts by draining the
//! chunk's `*Incoming` buffer into `queues.increase_queue` via
//! `pack_bfs_entry(..., FLAG_WRITE_LEVEL)`. Each inbox `CrossChunkWavefront`
//! encodes the source-frame face plus its on-face `(cell_x, cell_z)` and
//! `level`; the helper `face_cell_to_chunk_xyz` decodes those to the
//! destination-chunk-local `(x, y, z)` cell coordinates expected by the
//! packed BFS entry layout. The decoded face is inverted at distribute
//! time, so a wavefront arriving on the destination's West-`Incoming` lives
//! at `x = 0` inside the destination, and the BFS picks it up as if it had
//! been seeded at that cell from level `level`.

use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL};
use crate::distribute::direction_from_index;
use crate::geom::face_cell_to_chunk_xyz;
use crate::CrossChunkWavefront;

/// Drain a `*Incoming` buffer into a queues's `increase_queue` via
/// `pack_bfs_entry(..., FLAG_WRITE_LEVEL)`. Each entry is packed at the
/// destination-chunk-local cell decoded by `face_cell_to_chunk_xyz`, and
/// the BFS will write the wavefront's `level` and propagate outward from
/// there.
#[inline]
pub(crate) fn drain_incoming_into_queue(
    inbox: &mut smallvec::SmallVec<[CrossChunkWavefront; 16]>,
    queue: &mut Vec<u64>,
) {
    queue.reserve(inbox.len());
    for wavefront in inbox.drain(..) {
        let face = direction_from_index(wavefront.face());
        let (x, y, z) =
            face_cell_to_chunk_xyz(face, wavefront.cell_x(), wavefront.cell_z());
        queue.push(pack_bfs_entry(
            x,
            z,
            y,
            wavefront.level(),
            ALL_DIRECTIONS_BITSET,
            FLAG_WRITE_LEVEL,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_WRITE_LEVEL};
    use crate::block_light::enqueue::enqueue_block_light_on_block_placed;
    use crate::block_light::propagate::{
        propagate_decrease_block_system, propagate_increase_block_system,
    };
    use crate::codec::LightStorage;
    use crate::nibble::LightNibbles;
    use crate::sky_light::enqueue::enqueue_sky_light_on_block_placed;
    use crate::sky_light::propagate::{propagate_decrease_sky_system, propagate_increase_sky_system};
    use crate::table::{flag_bits, BlockStateLightTable};
    use crate::{
        BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockOutbox, IsAllAir,
        SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyOutbox,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::prelude::{Entity, IntoScheduleConfigs};
    use mcrs_core::voxel_shape::{Direction, VoxelShape};
    use mcrs_minecraft_block::palette::BlockPalette;
    use mcrs_protocol::BlockStateId;

    const AIR: BlockStateId = BlockStateId(0);
    const TORCH: BlockStateId = BlockStateId(1);

    fn make_test_table() -> BlockStateLightTable {
        let state_count = 2usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();
        emission[AIR.0 as usize] = 0;
        dampening[AIR.0 as usize] = 0;
        flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
        emission[TORCH.0 as usize] = 14;
        dampening[TORCH.0 as usize] = 0;
        flags[TORCH.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
        BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    fn air_palette() -> BlockPalette {
        let mut p = BlockPalette::default();
        p.fill(AIR);
        p
    }

    fn zero_light_storage() -> LightStorage {
        LightStorage::Dense(Box::new(LightNibbles::zeros()))
    }

    /// Pre-fill an L1 attenuated field for an emitter at (ex, ey, ez) with
    /// emission `e`, so a subsequent decrease pass has something to drain.
    fn seed_l1_field(light: &mut LightStorage, ex: i32, ey: i32, ez: i32, e: u8) {
        for y in 0..16i32 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let dist = ((x - ex).abs() + (y - ey).abs() + (z - ez).abs()) as u8;
                    let lvl = e.saturating_sub(dist);
                    if lvl > 0 {
                        light.set(x as usize, y as usize, z as usize, lvl);
                    }
                }
            }
        }
    }

    fn build_app_with_increase() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_increase_block_system);
        app
    }

    fn build_app_with_decrease() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_decrease_block_system);
        app
    }

    fn spawn_chunk_dirty(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockBfsQueues::default(),
                BlockOutbox::default(),
                BlockInbox::default(),
                BlockBfsPending,
            ))
            .id()
    }

    fn spawn_chunk_clean(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockBfsQueues::default(),
                BlockOutbox::default(),
                BlockInbox::default(),
            ))
            .id()
    }

    fn push_increase(app: &mut App, entity: Entity, entry: u64) {
        let mut ws = app
            .world_mut()
            .get_mut::<BlockBfsQueues>(entity)
            .expect("queues");
        ws.increase_queue.push(entry);
    }

    fn push_decrease(app: &mut App, entity: Entity, entry: u64) {
        let mut ws = app
            .world_mut()
            .get_mut::<BlockBfsQueues>(entity)
            .expect("queues");
        ws.decrease_queue.push(entry);
    }

    #[test]
    fn propagate_decrease_drains_queue() {
        let mut app = build_app_with_decrease();
        let entity = spawn_chunk_dirty(&mut app);
        let mut storage = zero_light_storage();
        seed_l1_field(&mut storage, 8, 8, 8, 14);
        storage.set(8, 8, 8, 0);
        app.world_mut()
            .get_mut::<BlockLight>(entity)
            .expect("BlockLight")
            .0 = storage;
        push_decrease(
            &mut app,
            entity,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let ws = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert!(
            ws.decrease_queue.is_empty(),
            "decrease_queue must drain to empty"
        );
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "decrease system does not clear BlockBfsPending (that is the increase system's job)"
        );
    }

    #[test]
    fn propagate_increase_drains_queue() {
        let mut app = build_app_with_increase();
        let entity = spawn_chunk_dirty(&mut app);
        app.world_mut()
            .get_mut::<BlockLight>(entity)
            .expect("BlockLight")
            .0
            .set(8, 8, 8, 14);
        push_increase(
            &mut app,
            entity,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let ws = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert!(
            ws.increase_queue.is_empty(),
            "increase_queue must drain to empty"
        );
        let light = app
            .world()
            .get::<BlockLight>(entity)
            .expect("BlockLight");
        assert_eq!(light.0.get(8, 8, 8), 14, "source cell unchanged");
        assert!(
            light.0.get(7, 8, 8) > 0 || light.0.get(9, 8, 8) > 0,
            "BFS must have written at least one neighbour"
        );
    }

    #[test]
    fn propagate_clears_light_dirty_when_drained() {
        let mut app = build_app_with_increase();
        let entity = spawn_chunk_dirty(&mut app);
        app.world_mut()
            .get_mut::<BlockLight>(entity)
            .expect("BlockLight")
            .0
            .set(8, 8, 8, 14);
        push_increase(
            &mut app,
            entity,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending cleared when both queues empty"
        );
    }

    #[test]
    fn propagate_clears_light_dirty_with_egress_nonempty() {
        let mut app = build_app_with_increase();
        let entity = spawn_chunk_dirty(&mut app);
        app.world_mut()
            .get_mut::<BlockLight>(entity)
            .expect("BlockLight")
            .0
            .set(15, 8, 8, 14);
        push_increase(
            &mut app,
            entity,
            pack_bfs_entry(15, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let ws = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        let outbox = app
            .world()
            .get::<BlockOutbox>(entity)
            .expect("BlockOutbox");
        assert!(ws.increase_queue.is_empty(), "queues drained");
        assert!(ws.decrease_queue.is_empty(), "queues drained");
        assert!(
            !outbox.0.is_empty(),
            "outbox must contain at least one East face wavefront"
        );
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending cleared even when BlockOutbox is non-empty"
        );
    }

    #[test]
    fn propagate_skips_clean_chunks() {
        let mut app = build_app_with_increase();
        let entity = spawn_chunk_clean(&mut app);
        push_increase(
            &mut app,
            entity,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let ws = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "queue NOT drained — clean chunk is skipped by With<BlockBfsPending>"
        );
        let light = app
            .world()
            .get::<BlockLight>(entity)
            .expect("BlockLight");
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(
                        light.0.get(x, y, z),
                        0,
                        "no cell should be written on a clean chunk"
                    );
                }
            }
        }
    }

    #[test]
    fn propagate_only_runs_on_dirty_chunks() {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(
            Update,
            (
                propagate_decrease_block_system,
                propagate_increase_block_system,
            )
                .chain(),
        );

        let dirty = spawn_chunk_dirty(&mut app);
        let clean = spawn_chunk_clean(&mut app);

        app.world_mut()
            .get_mut::<BlockLight>(dirty)
            .expect("BlockLight")
            .0
            .set(8, 8, 8, 14);
        push_increase(
            &mut app,
            dirty,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );
        push_increase(
            &mut app,
            clean,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let dirty_ws = app
            .world()
            .get::<BlockBfsQueues>(dirty)
            .expect("queues");
        let clean_ws = app
            .world()
            .get::<BlockBfsQueues>(clean)
            .expect("queues");
        assert!(
            dirty_ws.increase_queue.is_empty(),
            "dirty chunk's queue drained"
        );
        assert_eq!(
            clean_ws.increase_queue.len(),
            1,
            "clean chunk untouched — stale seed still in queue"
        );
        assert!(
            app.world().get::<BlockBfsPending>(dirty).is_none(),
            "dirty chunk's BlockBfsPending cleared"
        );
    }

    // -------- sky propagate system tests --------

    fn build_app_with_sky_increase() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_increase_sky_system);
        app
    }

    fn spawn_sky_chunk_all_air_with_top_seeds(app: &mut App) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(LightStorage::default()),
                SkyBfsQueues::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                IsAllAir,
                SkyBfsPending,
            ))
            .id();

        let mut ws = app
            .world_mut()
            .get_mut::<SkyBfsQueues>(entity)
            .expect("SkyBfsQueues");
        for z in 0..16u8 {
            for x in 0..16u8 {
                ws.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_WRITE_LEVEL,
                ));
            }
        }
        entity
    }

    fn spawn_sky_chunk_partial_air_with_top_seeds(app: &mut App) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(LightStorage::default()),
                SkyBfsQueues::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                SkyBfsPending,
            ))
            .id();

        let mut ws = app
            .world_mut()
            .get_mut::<SkyBfsQueues>(entity)
            .expect("SkyBfsQueues");
        for z in 0..16u8 {
            for x in 0..16u8 {
                ws.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_WRITE_LEVEL,
                ));
            }
        }
        entity
    }

    #[test]
    fn propagate_sky_column_walker_collapses_all_air() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_chunk_all_air_with_top_seeds(&mut app);

        app.update();

        let light = app
            .world()
            .get::<SkyLight>(entity)
            .expect("SkyLight");
        assert!(
            matches!(light.0, LightStorage::Uniform(15)),
            "column-walker must collapse the all-air chunk to Uniform(15); got {:?}",
            light.0
        );
        let ws = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("SkyBfsQueues");
        assert!(
            ws.increase_queue.is_empty(),
            "column-walker must clear the increase_queue"
        );
    }

    #[test]
    fn propagate_sky_column_walker_pushes_1280_wavefronts() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_chunk_all_air_with_top_seeds(&mut app);

        app.update();

        let outbox = app
            .world()
            .get::<SkyOutbox>(entity)
            .expect("SkyOutbox");
        assert_eq!(
            outbox.0.len(),
            1280,
            "column-walker must push 1280 wavefronts (5 non-Up faces x 256 cells)"
        );

        let first = outbox.0[0];
        assert!(
            matches!(first.face(), 0 | 2 | 3 | 4 | 5),
            "wavefront face must be one of the five non-Up faces; got {}",
            first.face()
        );
        assert_eq!(first.level(), 15, "wavefront level must be 15");
    }

    #[test]
    fn propagate_sky_column_walker_face_coordinates() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_chunk_all_air_with_top_seeds(&mut app);

        app.update();

        let outbox = app
            .world()
            .get::<SkyOutbox>(entity)
            .expect("SkyOutbox");
        assert_eq!(outbox.0.len(), 1280, "column-walker must push 1280 wavefronts");

        let west_face = Direction::West.index() as u8;
        let actual: Vec<(u8, u8)> = outbox
            .0
            .iter()
            .filter(|w| w.face() == west_face)
            .map(|w| (w.cell_x(), w.cell_z()))
            .collect();
        assert_eq!(actual.len(), 256, "expected 256 West-face wavefronts");

        let mut expected: Vec<(u8, u8)> = Vec::with_capacity(256);
        for a in 0..16u8 {
            for b in 0..16u8 {
                expected.push((a, b));
            }
        }
        assert_eq!(
            actual, expected,
            "West-face cells must follow (cell_x=a, cell_z=b) where a is outer and b is inner"
        );
    }

    #[test]
    fn propagate_sky_column_walker_skips_partial_air() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_chunk_partial_air_with_top_seeds(&mut app);

        app.update();

        let outbox = app
            .world()
            .get::<SkyOutbox>(entity)
            .expect("SkyOutbox");
        let up_face_count = outbox.0.iter().filter(|w| w.face() == 1).count();
        assert!(
            up_face_count > 0,
            "BFS path must push Up-face wavefronts; column-walker fast path excludes Up. outbox.len()={}, up_face_count={}",
            outbox.0.len(),
            up_face_count
        );
        assert_ne!(
            outbox.0.len(),
            1280,
            "BFS path produces a different wavefront count than the column-walker's exact 1280"
        );
    }

    #[test]
    fn propagate_sky_skyless_dim_iterates_nothing() {
        let mut app = build_app_with_sky_increase();
        let chunk = app
            .world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockBfsQueues::default(),
                BlockOutbox::default(),
                BlockInbox::default(),
                SkyBfsPending,
            ))
            .id();

        app.update();

        assert!(
            app.world().entity(chunk).get::<SkyLight>().is_none(),
            "skyless-dim chunk must never gain SkyLight from the sky propagate systems"
        );
        assert!(
            app.world().entity(chunk).get::<SkyOutbox>().is_none(),
            "skyless-dim chunk must never gain SkyOutbox from the sky propagate systems"
        );
    }

    /// Verify drain-Incoming prelude turns one BlockInbox wavefront into a
    /// pack_bfs_entry FLAG_WRITE_LEVEL on queues.increase_queue. East face
    /// wavefront at (cell_x=0, cell_z=8, level=8) decodes to chunk-local
    /// (x=15, y=0, z=8) per face_cell_to_chunk_xyz(East, 0, 8). The drain
    /// runs at the top of `propagate_decrease_block_system`; the decrease
    /// pass itself only drains `decrease_queue`, so the prelude's
    /// FLAG_WRITE_LEVEL entry survives on `increase_queue` for the next
    /// increase pass to consume.
    #[test]
    fn propagate_decrease_drains_block_incoming_at_top_of_body() {
        let mut app = build_app_with_decrease();
        let entity = spawn_chunk_dirty(&mut app);
        let east = Direction::East.index() as u8;
        let mut inbox = app
            .world_mut()
            .get_mut::<BlockInbox>(entity)
            .expect("inbox");
        inbox.0.push(CrossChunkWavefront::new(east, 0, 8, 8));
        drop(inbox);

        app.update();

        let inc = app
            .world()
            .get::<BlockInbox>(entity)
            .expect("inbox");
        assert!(inc.0.is_empty(), "drain prelude must empty BlockInbox");
        let ws = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "drain prelude wrote exactly one entry onto increase_queue; decrease pass does not drain that queue"
        );
        let entry = ws.increase_queue[0];
        assert_eq!(crate::bfs::unpack_bfs_entry_x(entry), 15);
        assert_eq!(crate::bfs::unpack_bfs_entry_y(entry) as u8 & 0xF, 0);
        assert_eq!(crate::bfs::unpack_bfs_entry_z(entry), 8);
        assert_eq!(crate::bfs::unpack_bfs_entry_level(entry), 8);
        assert_ne!(
            crate::bfs::unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "FLAG_WRITE_LEVEL must be set on the packed entry"
        );
    }

    /// Same shape for sky-side. South face wavefront at (cell_x=4, cell_z=7,
    /// level=12) decodes to chunk-local (x=4, y=7, z=15) per
    /// face_cell_to_chunk_xyz(South, 4, 7).
    #[test]
    fn propagate_increase_drains_sky_incoming_at_top_of_body() {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_decrease_sky_system);
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(zero_light_storage()),
                SkyBfsQueues::default(),
                SkyOutbox::default(),
                SkyInbox::default(),
                SkyBfsPending,
            ))
            .id();
        let south = Direction::South.index() as u8;
        let mut inc = app
            .world_mut()
            .get_mut::<SkyInbox>(entity)
            .expect("inbox");
        inc.0.push(CrossChunkWavefront::new(south, 4, 7, 12));
        drop(inc);

        app.update();

        let inc = app
            .world()
            .get::<SkyInbox>(entity)
            .expect("inbox");
        assert!(inc.0.is_empty(), "drain prelude must empty SkyInbox");
        let ws = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("queues");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "drain prelude wrote one entry onto increase_queue; decrease pass does not drain that queue"
        );
    }

    /// Spawn 100 chunks each with BlockBfsPending + a seeded BlockLight cell
    /// + an increase_queue entry, then run propagate_increase_block_system
    /// which uses par_iter_mut. Assert all 100 chunks drain their queues
    /// and clear BlockBfsPending. This is structural: par_iter_mut must
    /// compile, run without deadlock, and produce identical results to iter_mut.
    #[test]
    fn propagate_decrease_runs_under_par_iter_mut() {
        let mut app = build_app_with_increase();
        let mut entities = Vec::with_capacity(100);
        for _ in 0..100 {
            let e = spawn_chunk_dirty(&mut app);
            app.world_mut()
                .get_mut::<BlockLight>(e)
                .expect("BlockLight")
                .0
                .set(8, 8, 8, 14);
            push_increase(
                &mut app,
                e,
                pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
            );
            entities.push(e);
        }

        app.update();

        for e in entities {
            let ws = app
                .world()
                .get::<BlockBfsQueues>(e)
                .expect("queues");
            assert!(
                ws.increase_queue.is_empty(),
                "entity {e:?} queue drained under par_iter_mut"
            );
            assert!(
                app.world().get::<BlockBfsPending>(e).is_none(),
                "entity {e:?} BlockBfsPending cleared under par_iter_mut"
            );
        }
    }

    // ---- per-channel marker isolation tests ----

    #[test]
    fn block_only_event_inserts_only_block_bfs_pending() {
        use bevy_ecs::message::Messages;
        use mcrs_engine::world::block::BlockPos;
        use mcrs_engine::world::chunk::ChunkPos;
        use mcrs_minecraft_block::block::BlockUpdateFlags;
        use mcrs_minecraft_block::block_update::BlockPlaced;

        const TORCH: BlockStateId = BlockStateId(1);

        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(
            Update,
            (
                enqueue_block_light_on_block_placed,
                enqueue_sky_light_on_block_placed,
            ),
        );

        let chunk = app
            .world_mut()
            .spawn((BlockLight::default(), BlockBfsQueues::default()))
            .id();

        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(BlockPlaced {
                chunk,
                chunk_pos: ChunkPos::new(0, 0, 0),
                block_pos: BlockPos::new(3, 5, 9),
                old_state: AIR,
                new_state: TORCH,
                flags: BlockUpdateFlags::empty(),
            });

        app.update();

        assert!(
            app.world().get::<BlockBfsPending>(chunk).is_some(),
            "block-emitter change must insert BlockBfsPending on the chunk"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_none(),
            "block-only event must NOT insert SkyBfsPending on a chunk lacking SkyLight"
        );
    }

    #[test]
    fn sky_only_opacity_change_inserts_only_sky_bfs_pending() {
        use bevy_ecs::message::Messages;
        use mcrs_engine::world::block::BlockPos;
        use mcrs_engine::world::chunk::ChunkPos;
        use mcrs_engine::world::column::{ColumnChunks, InColumn};
        use mcrs_minecraft_block::block::BlockUpdateFlags;
        use mcrs_minecraft_block::block_update::BlockPlaced;

        const LEAVES: BlockStateId = BlockStateId(4);

        let state_count = 5usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();
        emission[AIR.0 as usize] = 0;
        dampening[AIR.0 as usize] = 0;
        flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
        emission[LEAVES.0 as usize] = 0;
        dampening[LEAVES.0 as usize] = 1;
        flags[LEAVES.0 as usize] = 0;

        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        });
        app.add_systems(
            Update,
            (
                enqueue_block_light_on_block_placed,
                enqueue_sky_light_on_block_placed,
            ),
        );

        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            BlockLight::default(),
            BlockBfsQueues::default(),
            SkyLight::default(),
            SkyBfsQueues::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));

        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(BlockPlaced {
                chunk,
                chunk_pos: ChunkPos::new(0, 0, 0),
                block_pos: BlockPos::new(8, 10, 8),
                old_state: AIR,
                new_state: LEAVES,
                flags: BlockUpdateFlags::empty(),
            });

        app.update();

        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_some(),
            "opacity-only change must insert SkyBfsPending on the chunk"
        );
        assert!(
            app.world().get::<BlockBfsPending>(chunk).is_none(),
            "dampening-only change must NOT insert BlockBfsPending (block enqueue skips)"
        );
    }
}
