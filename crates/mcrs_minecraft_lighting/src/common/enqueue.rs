//! Consumes `MessageReader<BlockPlaced>`, derives the chunk's intra-cell
//! coord via `rem_euclid(16)` on i32, looks up old/new emission via
//! `Res<BlockStateLightTable>`, and pushes a decrease and/or increase seed into
//! the chunk's `BlockBfsQueues` queues per the emission-diff rule:
//! `old_emission > new_emission` → decrease seed at `old_emission`,
//! `new_emission > 0` → increase seed at `new_emission`. `BlockBfsPending`
//! is inserted via `Commands::entity(placed.chunk).insert(BlockBfsPending)`
//! when at least one seed was pushed; the sky-channel counterpart inserts
//! `SkyBfsPending` analogously.
//!
//! Dampening-only changes (`old_emission == new_emission &&
//! old_dampening != new_dampening`) emit a `tracing::warn!` and skip; the
//! cell will desync until cross-chunk distribute lands. Missing
//! `BlockLight`/`BlockBfsQueues` components on the message's
//! `chunk` entity also emit a warning and skip — defensive against any
//! lifecycle-ordering hazard.

use bevy_ecs::prelude::{Added, Commands, Entity, Query, With};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::column::{Column, ColumnChunks};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};

use crate::{BlockNeedsInitialSeed, NeedsFullReseed, SkyNeedsInitialSeed};

pub(crate) const CARDINAL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Consumes `Added<NeedsFullReseed>` on `Column` entities: iterates the
/// column's `ColumnChunks.sections` slots and re-inserts
/// `BlockNeedsInitialSeed` unconditionally plus `SkyNeedsInitialSeed` on
/// chunks whose dimension carries `HasSkyLight`, on every loaded chunk in the
/// column. Removes `NeedsFullReseed` from the column entity.
///
/// Gated on `ColumnHeightmapScan::is_finalized()`. When the scan is not yet
/// finalized, `Heightmaps::surface_get` returns the sentinel `min_y` for
/// every unclosed XZ column. Re-marking chunks with the per-channel markers
/// in that state causes `seed_sky_initial` to misclassify cave chunks as
/// Case A (Uniform(15)). The natural lifecycle in
/// `prime_heightmaps_on_column_spawn` inserts the markers once the scan
/// finalizes with a correctly primed heightmap, so deferring is safe.
pub fn consume_needs_full_reseed(
    newly_marked: Query<
        (Entity, &ColumnChunks, Option<&crate::lifecycle::ColumnHeightmapScan>),
        (With<Column>, Added<NeedsFullReseed>),
    >,
    in_dimensions: Query<&InDimension>,
    sky_dims: Query<(), With<HasSkyLight>>,
    mut commands: Commands,
) {
    for (column_entity, chunk_index, scan_opt) in newly_marked.iter() {
        let loaded = chunk_index.sections.iter().filter(|s| s.is_some()).count();
        let total = chunk_index.sections.len();
        let scan_finalized = scan_opt.map_or(false, |s| s.is_finalized());

        if !scan_finalized {
            tracing::warn!(
                target: "mcrs_lighting::consume_reseed",
                column = ?column_entity,
                chunks_loaded = loaded,
                chunks_total = total,
                scan_present = scan_opt.is_some(),
                "Dropping NeedsFullReseed: heightmap scan not finalized. The natural lifecycle in \
                 prime_heightmaps_on_column_spawn will insert the per-channel needs-initial markers \
                 when the scan closes; reseeding now would read sentinel min_y and mis-Uniform(15) cave chunks."
            );
            commands.entity(column_entity).remove::<NeedsFullReseed>();
            continue;
        }

        for slot in chunk_index.sections.iter() {
            if let Some(chunk_entity) = slot {
                let mut e = commands.entity(*chunk_entity);
                e.insert(BlockNeedsInitialSeed);
                if let Ok(in_dim) = in_dimensions.get(*chunk_entity) {
                    if sky_dims.get(in_dim.0).is_ok() {
                        e.insert(SkyNeedsInitialSeed);
                    }
                }
            }
        }
        commands.entity(column_entity).remove::<NeedsFullReseed>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{
        unpack_bfs_entry_flags, unpack_bfs_entry_level, unpack_bfs_entry_x, unpack_bfs_entry_y,
        unpack_bfs_entry_z, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL,
    };
    use crate::block_light::enqueue::{
        enqueue_block_light_on_block_placed, pull_block_neighbor_edges, seed_block_emitters,
    };
    use crate::sky_light::components::NeedsRetop;
    use crate::sky_light::enqueue::{
        enqueue_sky_light_on_block_placed, invalidate_previous_topmost, pull_sky_neighbor_edges,
        seed_sky_initial,
    };
    use crate::table::{flag_bits, BlockStateLightTable};
    use crate::{
        BlockBfsPending, BlockBfsQueues, BlockInbox, BlockLight, BlockParkedEgress,
        CrossChunkWavefront, SkyBfsPending, SkyBfsQueues, SkyInbox, SkyLight, SkyParkedEgress,
        WasTopmostAtSeed,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::IntoScheduleConfigs;
    use mcrs_core::voxel_shape::{Direction, VoxelShape};
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
    use mcrs_engine::world::column::{
        Column, ColumnChunks, ColumnIndex, ColumnPos, ColumnSlot, InColumn,
    };
    use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
    use mcrs_lighting_table_helpers::*;
    use mcrs_minecraft_block::block::BlockUpdateFlags;
    use mcrs_minecraft_block::block_update::BlockPlaced;
    use mcrs_minecraft_block::palette::BlockPalette;
    use mcrs_protocol::BlockStateId;

    mod mcrs_lighting_table_helpers {
        use super::*;
        use crate::table::{flag_bits, BlockStateLightTable};

        pub const AIR: BlockStateId = BlockStateId(0);
        pub const STONE: BlockStateId = BlockStateId(1);
        pub const TORCH_HI: BlockStateId = BlockStateId(2);
        pub const TORCH_LO: BlockStateId = BlockStateId(3);
        pub const LEAVES: BlockStateId = BlockStateId(4);

        pub fn make_test_table() -> BlockStateLightTable {
            let state_count = 5usize;
            let mut emission = vec![0u8; state_count].into_boxed_slice();
            let mut dampening = vec![0u8; state_count].into_boxed_slice();
            let occlusion: Box<[&'static VoxelShape]> =
                vec![VoxelShape::empty(); state_count].into_boxed_slice();
            let mut flags = vec![0u8; state_count].into_boxed_slice();

            emission[AIR.0 as usize] = 0;
            dampening[AIR.0 as usize] = 0;
            flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[STONE.0 as usize] = 0;
            dampening[STONE.0 as usize] = 0;
            flags[STONE.0 as usize] = 0;

            emission[TORCH_HI.0 as usize] = 14;
            dampening[TORCH_HI.0 as usize] = 0;
            flags[TORCH_HI.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[TORCH_LO.0 as usize] = 7;
            dampening[TORCH_LO.0 as usize] = 0;
            flags[TORCH_LO.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[LEAVES.0 as usize] = 0;
            dampening[LEAVES.0 as usize] = 1;
            flags[LEAVES.0 as usize] = flag_bits::IS_NOT_AIR;

            BlockStateLightTable {
                emission,
                dampening,
                occlusion,
                flags,
            }
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_block_light_on_block_placed);
        app
    }

    fn spawn_chunk(app: &mut App) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((BlockLight::default(), BlockBfsQueues::default()))
            .id()
    }

    fn write_placed(app: &mut App, placed: BlockPlaced) {
        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(placed);
    }

    fn block_placed(
        chunk: bevy_ecs::entity::Entity,
        block_pos: BlockPos,
        old_state: BlockStateId,
        new_state: BlockStateId,
    ) -> BlockPlaced {
        BlockPlaced {
            chunk,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos,
            old_state,
            new_state,
            flags: BlockUpdateFlags::empty(),
        }
    }

    #[test]
    fn enqueue_increase_on_emitter_placed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 5, 9), AIR, TORCH_HI),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(queues.increase_queue.len(), 1, "one increase seed");
        assert!(
            queues.decrease_queue.is_empty(),
            "no decrease seed for 0 → 14"
        );
        let entry = queues.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_some(),
            "BlockBfsPending inserted"
        );
    }

    #[test]
    fn enqueue_decrease_on_emitter_removed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 8, 8), TORCH_HI, AIR),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(queues.decrease_queue.len(), 1, "one decrease seed");
        assert!(
            queues.increase_queue.is_empty(),
            "no increase seed for 14 → 0"
        );
        let entry = queues.decrease_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 8);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 8);
        assert_eq!(unpack_bfs_entry_z(entry), 8);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(app.world().get::<BlockBfsPending>(entity).is_some());
    }

    #[test]
    fn enqueue_both_on_swap() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 2, 3), TORCH_HI, TORCH_LO),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(queues.decrease_queue.len(), 1);
        assert_eq!(queues.increase_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(queues.decrease_queue[0]),
            14,
            "decrease at old emission"
        );
        assert_eq!(
            unpack_bfs_entry_level(queues.increase_queue[0]),
            7,
            "increase at new emission"
        );
        assert!(app.world().get::<BlockBfsPending>(entity).is_some());
    }

    #[test]
    fn enqueue_no_op_on_zero_zero() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR → STONE: both emission=0, both dampening=0 in the test table, so
        // the dampening-only-change branch does NOT trigger and the system
        // simply records no work.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, STONE),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert!(queues.increase_queue.is_empty());
        assert!(queues.decrease_queue.is_empty());
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending NOT inserted on no-op"
        );
    }

    #[test]
    fn enqueue_dampening_only_change_warns() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR (emission=0, dampening=0) → LEAVES (emission=0, dampening=1).
        // Pure dampening change; the system warns and skips.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, LEAVES),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert!(
            queues.increase_queue.is_empty(),
            "dampening-only skips increase"
        );
        assert!(
            queues.decrease_queue.is_empty(),
            "dampening-only skips decrease"
        );
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending NOT inserted on dampening-only change"
        );
    }

    #[test]
    fn enqueue_missing_components_warns() {
        let mut app = build_app();
        // Spawn an entity WITHOUT BlockLight/BlockBfsQueues — emulates
        // a chunk the lighting lifecycle has not yet attached state to.
        let entity = app.world_mut().spawn(()).id();
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, TORCH_HI),
        );

        app.update();

        assert!(
            app.world().get::<BlockBfsQueues>(entity).is_none(),
            "entity still has no queues"
        );
        assert!(
            app.world().get::<BlockBfsPending>(entity).is_none(),
            "BlockBfsPending must NOT be inserted on missing components"
        );
    }

    #[test]
    fn enqueue_uses_rem_euclid_for_negative_coords() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // BlockPos::new(-3, 5, -19) — rem_euclid(16) yields (13, 5, 13).
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(-3, 5, -19), AIR, TORCH_HI),
        );

        app.update();

        let queues = app
            .world()
            .get::<BlockBfsQueues>(entity)
            .expect("queues");
        assert_eq!(queues.increase_queue.len(), 1);
        let entry = queues.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 13, "x = -3 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 13, "z = -19 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_level(entry), 14);
    }

    fn build_sky_initial_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, seed_sky_initial);
        app
    }

    fn spawn_fallback_dim(app: &mut App) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn(ColumnIndex::default());
        e.insert(HasSkyLight);
        e.id()
    }

    fn air_palette_local() -> BlockPalette {
        let mut p = BlockPalette::default();
        p.fill(AIR);
        p
    }

    /// Fallback branch: a topmost-of-column chunk freshly added to a
    /// sky-having dim (no `SkyNeedsInitialSeed` marker; no primed heightmap on
    /// the column) must seed 256 entries via the `Added<SkyLight>` arm.
    #[test]
    fn seed_sky_initial_seeds_topmost_chunk_only_via_fallback() {
        let mut app = build_sky_initial_app();
        let dim = spawn_fallback_dim(&mut app);

        let chunk = app.world_mut().spawn_empty().id();
        // Anchor the column on the dim and add the column to the dim's index
        // so `seed_sky_initial`'s dim-has-sky probe finds the dim.
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        app.world_mut().entity_mut(chunk).insert((
            air_palette_local(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            SkyLight::default(),
            SkyBfsQueues::default(),
        ));

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(chunk)
            .expect("sky queues");
        assert_eq!(
            queues.increase_queue.len(),
            256,
            "topmost-of-column chunk seeds 256 entries (16 x 16 at y=15)"
        );
        assert!(
            queues.decrease_queue.is_empty(),
            "initial seed does not push decrease entries"
        );
        for entry in &queues.increase_queue {
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15, "y == 15");
            assert_eq!(unpack_bfs_entry_level(*entry), 15, "level == 15");
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "FLAG_WRITE_LEVEL bit set on every seed"
            );
        }
        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_some(),
            "SkyBfsPending inserted on topmost-of-column seed"
        );
    }

    /// Counterpart to the fallback test: a non-topmost chunk freshly added to
    /// the same sky-having dim must not seed 256 entries.
    #[test]
    fn seed_sky_initial_skips_non_topmost_via_fallback() {
        let mut app = build_sky_initial_app();
        let dim = spawn_fallback_dim(&mut app);

        let chunk_below = app.world_mut().spawn_empty().id();
        let chunk_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_below), Some(chunk_topmost)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        // Only the below chunk gets SkyLight added; topmost is left bare
        // so this single test does not also seed an unrelated chunk.
        app.world_mut().entity_mut(chunk_below).insert((
            air_palette_local(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            SkyLight::default(),
            SkyBfsQueues::default(),
        ));

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(chunk_below)
            .expect("sky queues");
        assert!(
            queues.increase_queue.is_empty(),
            "non-topmost chunk seeds nothing"
        );
        assert!(
            queues.decrease_queue.is_empty(),
            "non-topmost chunk seeds no decrease"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk_below).is_none(),
            "SkyBfsPending NOT inserted on non-topmost-of-column chunk"
        );
    }

    fn build_sky_on_placed_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        app
    }

    fn spawn_sky_chunk_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyBfsQueues::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    fn spawn_sky_chunk_non_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let dummy_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk), Some(dummy_topmost)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyBfsQueues::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    #[test]
    fn enqueue_sky_on_block_placed_pushes_decrease_and_neighbour_seeds() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR (damp=0, propagates) -> LEAVES (damp=1, no propagates flag);
        // sky_changed predicate trips on both dampening and flag delta.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 10, 8), AIR, LEAVES),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert!(
            !queues.decrease_queue.is_empty(),
            "dampening change pushes a decrease seed"
        );
        assert!(
            !queues.increase_queue.is_empty(),
            "y=10 (non-top) pushes neighbour-support increase seeds"
        );
        // y=10 (intra-chunk, not 15) -> six neighbour seeds.
        assert_eq!(
            queues.increase_queue.len(),
            6,
            "y < 15 produces exactly six neighbour-support seeds"
        );
        for entry in &queues.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
        }
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_some(),
            "SkyBfsPending inserted after dampening change"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_top_seeds_top_face() {
        // y == 15 path: a single top-face increase seed instead of six
        // neighbour seeds.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert_eq!(
            queues.increase_queue.len(),
            1,
            "y == 15 produces exactly one top-face seed"
        );
        let entry = queues.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-of-chunk seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_when_predicate_false() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR -> AIR: old_state == new_state, early-out before predicate.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, AIR),
        );
        // AIR -> TORCH_HI: both have dampening=0 AND PROPAGATES_SKYLIGHT_DOWN,
        // so sky_changed is false and the system continues without queueing.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 1, 1), AIR, TORCH_HI),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert!(queues.increase_queue.is_empty());
        assert!(queues.decrease_queue.is_empty());
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_none(),
            "SkyBfsPending NOT inserted on no-op sky enqueue"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_warns_missing_components() {
        use std::io;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone)]
        struct VecWriter(Arc<Mutex<Vec<u8>>>);

        impl io::Write for VecWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl<'a> MakeWriter<'a> for VecWriter {
            type Writer = VecWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let writer = VecWriter(Arc::clone(&captured));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut app = build_sky_on_placed_app();
            // Chunk without SkyLight/SkyBfsQueues (skyless-dim shape).
            let entity = app
                .world_mut()
                .spawn((BlockLight::default(), BlockBfsQueues::default()))
                .id();
            write_placed(
                &mut app,
                block_placed(entity, BlockPos::new(2, 3, 4), AIR, LEAVES),
            );

            app.update();

            assert!(
                app.world().get::<SkyBfsQueues>(entity).is_none(),
                "entity still has no sky queues"
            );
            assert!(
                app.world().get::<SkyBfsPending>(entity).is_none(),
                "SkyBfsPending must NOT be inserted when SkyLight is missing"
            );
        });

        let bytes = captured.lock().unwrap();
        let output = String::from_utf8_lossy(&bytes);
        assert!(
            output.contains("BlockPlaced.chunk missing SkyLight/SkyBfsQueues"),
            "expected warn substring in captured tracing output, got: {output}"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_clears_seed_cell_on_opacity_rise() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 10);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), AIR, LEAVES),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            0,
            "seed cell cleared because opacity rose"
        );
        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert_eq!(queues.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(queues.decrease_queue[0]),
            10,
            "decrease seed carries pre-clear stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_keeps_seed_cell_when_opacity_drops() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 3);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), LEAVES, AIR),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            3,
            "seed cell unchanged because opacity did not rise"
        );
        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert_eq!(queues.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(queues.decrease_queue[0]),
            3,
            "decrease seed carries stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_top_seed_when_not_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_non_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        // y=15 sits at the top of the chunk, so the Up neighbour at y=16
        // is outside the chunk and is skipped by the bounds guard. Five
        // neighbour-recheck seeds remain.
        assert_eq!(
            queues.increase_queue.len(),
            5,
            "non-topmost chunk falls through to neighbour-recheck branch at y=15"
        );
        for entry in &queues.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
            assert_eq!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "no neighbour seed carries FLAG_WRITE_LEVEL"
            );
        }
    }

    #[test]
    fn enqueue_sky_on_block_placed_emits_top_seed_when_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert_eq!(
            queues.increase_queue.len(),
            1,
            "topmost chunk emits a single top-face seed at y=15"
        );
        let entry = queues.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-face seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_trips_on_occlusion_only_change() {
        const SHAPE_A: BlockStateId = BlockStateId(10);
        const SHAPE_B: BlockStateId = BlockStateId(11);

        let state_count = 12usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let mut occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();

        emission[AIR.0 as usize] = 0;
        dampening[AIR.0 as usize] = 0;
        flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

        // Two states share dampening and flag bits but project distinct
        // occlusion shapes. `dampening = 5` keeps `PROPAGATES_SKYLIGHT_DOWN`
        // cleared on both (matching the production `compute_flags` invariant)
        // so the dampening and flag arms of `sky_changed` stay silent and the
        // test exclusively exercises the occlusion-shape pointer comparison.
        dampening[SHAPE_A.0 as usize] = 5;
        dampening[SHAPE_B.0 as usize] = 5;
        flags[SHAPE_A.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        flags[SHAPE_B.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        occlusion[SHAPE_A.0 as usize] = VoxelShape::empty();
        occlusion[SHAPE_B.0 as usize] = VoxelShape::block();

        let table = BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        };

        assert!(
            !std::ptr::eq(
                table.occlusion_for(SHAPE_A) as *const _,
                table.occlusion_for(SHAPE_B) as *const _,
            ),
            "fixture must mint distinct occlusion shape pointers"
        );
        assert_eq!(table.dampening_for(SHAPE_A), table.dampening_for(SHAPE_B));
        assert_eq!(
            table.flags_for(SHAPE_A) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
            table.flags_for(SHAPE_B) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        );

        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(table);
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), SHAPE_A, SHAPE_B),
        );

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(entity)
            .expect("sky queues");
        assert_eq!(
            queues.decrease_queue.len(),
            1,
            "occlusion-only delta still pushes a decrease seed"
        );
        assert_eq!(
            queues.increase_queue.len(),
            6,
            "y != 15 path enqueues six neighbour-recheck seeds"
        );
        assert!(
            app.world().get::<SkyBfsPending>(entity).is_some(),
            "occlusion-only delta inserts SkyBfsPending"
        );
    }

    fn build_seed_initial_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        // Register all three seed systems with the strict ordering used in
        // plugin.rs: (seed_block_emitters, seed_sky_initial) run together,
        // then `invalidate_previous_topmost` runs after `seed_sky_initial` so
        // the `NeedsRetop` handoff is visible to the consumer.
        app.add_systems(
            Update,
            (
                (seed_block_emitters, seed_sky_initial),
                invalidate_previous_topmost.after(seed_sky_initial),
            ),
        );
        app
    }

    fn spawn_palette_with_torches(positions: &[(i32, i32, i32)]) -> BlockPalette {
        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        for &(x, y, z) in positions {
            palette.set(BlockPos::new(x, y, z), TORCH_HI);
        }
        palette
    }

    fn spawn_dimension(app: &mut App, with_sky: bool) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn(ColumnIndex::default());
        if with_sky {
            e.insert(HasSkyLight);
        }
        e.id()
    }

    fn spawn_topmost_chunk_for_seed(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        palette: BlockPalette,
        sky: bool,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut emut = app.world_mut().entity_mut(chunk);
        emut.insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockBfsQueues::default(),
            BlockNeedsInitialSeed,
        ));
        if sky {
            emut.insert((
                SkyLight::default(),
                SkyBfsQueues::default(),
                SkyNeedsInitialSeed,
            ));
        }
        (chunk, column)
    }

    #[test]
    fn seed_block_emitters_and_sky_initial_emit_block_emitters_and_sky_source() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);
        let palette = spawn_palette_with_torches(&[
            (0, 0, 0),
            (5, 5, 5),
            (10, 1, 8),
            (3, 12, 7),
            (15, 15, 15),
        ]);
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, true);

        app.update();

        let block_ws = app
            .world()
            .get::<BlockBfsQueues>(chunk)
            .expect("block ws");
        assert_eq!(
            block_ws.increase_queue.len(),
            5,
            "five torches emit five increase seeds"
        );
        let sky_ws = app
            .world()
            .get::<SkyBfsQueues>(chunk)
            .expect("sky ws");
        assert_eq!(
            sky_ws.increase_queue.len(),
            256,
            "topmost on sky-having dim with absent Heightmaps falls back to 256 sky entries"
        );
        assert!(
            app.world().get::<WasTopmostAtSeed>(chunk).is_some(),
            "WasTopmostAtSeed inserted"
        );
        assert!(
            app.world().get::<BlockBfsPending>(chunk).is_some(),
            "BlockBfsPending inserted by seed_block_emitters"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_some(),
            "SkyBfsPending inserted by seed_sky_initial"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk).is_none(),
            "BlockNeedsInitialSeed removed by seed_block_emitters"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk).is_none(),
            "SkyNeedsInitialSeed removed by seed_sky_initial"
        );
    }

    /// Regression test: when a new chunk takes over as the column's topmost,
    /// the previous topmost gets the `NeedsRetop` handoff and
    /// `invalidate_previous_topmost` runs the decrease wave through its top
    /// face within the same tick chain.
    #[test]
    fn retopping_handoff_completes_in_one_tick() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Chunk A at chunk-Y 0 with WasTopmostAtSeed already; stored
        // sky level 12 across the top face.
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut palette_a = BlockPalette::default();
        palette_a.fill(AIR);
        let mut a_sky_light = SkyLight::default();
        for z in 0..16usize {
            for x in 0..16usize {
                a_sky_light.0.set(x, 15, z, 12);
            }
        }
        app.world_mut().entity_mut(chunk_a).insert((
            palette_a,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockBfsQueues::default(),
            a_sky_light,
            SkyBfsQueues::default(),
            WasTopmostAtSeed,
        ));

        // Chunk B at chunk-Y 1 (the new topmost) needs initial light. Both
        // per-channel markers are present to trigger seed_block_emitters and
        // seed_sky_initial.
        let mut palette_b = BlockPalette::default();
        palette_b.fill(AIR);
        app.world_mut().entity_mut(chunk_b).insert((
            palette_b,
            ChunkPos::new(0, 1, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockBfsQueues::default(),
            SkyLight::default(),
            SkyBfsQueues::default(),
            BlockNeedsInitialSeed,
            SkyNeedsInitialSeed,
        ));

        app.update();

        // Previous topmost A: marker removed, SkyBfsPending inserted, decrease
        // wave seeded with stored level 12.
        assert!(
            app.world().get::<WasTopmostAtSeed>(chunk_a).is_none(),
            "previous topmost's WasTopmostAtSeed removed by seed_sky_initial"
        );
        assert!(
            app.world().get::<NeedsRetop>(chunk_a).is_none(),
            "NeedsRetop consumed by invalidate_previous_topmost"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk_a).is_some(),
            "previous topmost marked SkyBfsPending"
        );
        let a_ws = app
            .world()
            .get::<SkyBfsQueues>(chunk_a)
            .expect("sky ws on A");
        assert_eq!(
            a_ws.decrease_queue.len(),
            256,
            "previous topmost gets 256 decrease seeds"
        );
        for entry in &a_ws.decrease_queue {
            assert_eq!(
                unpack_bfs_entry_level(*entry),
                12,
                "decrease seed carries stored level"
            );
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15);
        }

        // New topmost B: marker inserted, increase queue seeded.
        assert!(
            app.world().get::<WasTopmostAtSeed>(chunk_b).is_some(),
            "new topmost seeded"
        );
        let b_ws = app
            .world()
            .get::<SkyBfsQueues>(chunk_b)
            .expect("sky ws on B");
        assert_eq!(b_ws.increase_queue.len(), 256);
    }

    #[test]
    fn seed_sky_initial_skips_skyless_dim_for_sky_seed() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, false);
        let palette = spawn_palette_with_torches(&[(2, 2, 2)]);
        // Skyless dim: spawn the chunk without a SkyLight/SkyBfsQueues
        // (matching the skyless-dimension contract).
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, false);

        app.update();

        // Block-light emitter seed lands as usual.
        let block_ws = app
            .world()
            .get::<BlockBfsQueues>(chunk)
            .expect("block ws");
        assert_eq!(block_ws.increase_queue.len(), 1);

        // No sky queues was attached, so sky pathways are inert.
        assert!(
            app.world().get::<SkyBfsQueues>(chunk).is_none(),
            "skyless dim has no sky queues"
        );
        assert!(
            app.world().get::<WasTopmostAtSeed>(chunk).is_none(),
            "skyless dim chunk does not insert WasTopmostAtSeed"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_none(),
            "skyless-dim chunk must not be marked SkyBfsPending"
        );
        assert!(
            app.world().get::<BlockBfsPending>(chunk).is_some(),
            "block-light emitter seed still marks the chunk BlockBfsPending"
        );
    }

    /// Regression test: the `Added<SkyLight>` fallback
    /// branch of `seed_sky_initial` fires on a topmost-of-column chunk in a
    /// sky-having dim whose column has no primed heightmap. The fallback
    /// pushes 256 seeds even though `SkyNeedsInitialSeed` was never inserted.
    #[test]
    fn seed_sky_initial_fallback_branch_seeds_256_on_partial_load() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Single topmost chunk; no `SkyNeedsInitialSeed`, no Heightmaps on
        // the column. The `Added<SkyLight>` arm of the filter must fire.
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        app.world_mut().entity_mut(chunk).insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockBfsQueues::default(),
            SkyLight::default(),
            SkyBfsQueues::default(),
            // NOTE: no SkyNeedsInitialSeed — the fallback fires on Added<SkyLight>.
        ));

        app.update();

        let queues = app
            .world()
            .get::<SkyBfsQueues>(chunk)
            .expect("sky queues");
        assert_eq!(
            queues.increase_queue.len(),
            256,
            "fallback arm seeds 256 entries on topmost-of-column"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk).is_some(),
            "SkyBfsPending inserted by the fallback branch"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk).is_none(),
            "marker was never inserted; fallback path does not remove what isn't there"
        );
    }

    /// Regression test for the orphan-marker cleanup pass: a chunk carrying
    /// `NeedsRetop` but lacking `SkyLight` (skyless dim, or a non-storage
    /// entity that the producer's Err-branch fallback tagged conservatively)
    /// must have the marker stripped by `invalidate_previous_topmost` so it
    /// cannot accumulate across ticks.
    #[test]
    fn invalidate_previous_topmost_clears_orphan_needs_retop_on_non_sky_chunks() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        let orphan = app
            .world_mut()
            .spawn((
                ChunkPos::new(0, 0, 0),
                InColumn(dim),
                InDimension(dim),
                NeedsRetop,
            ))
            .id();

        app.update();

        assert!(
            app.world().get::<NeedsRetop>(orphan).is_none(),
            "non-sky chunk's NeedsRetop must be cleared by the cleanup pass"
        );
        assert!(
            app.world().get::<SkyBfsQueues>(orphan).is_none(),
            "cleanup pass must not synthesize a SkyBfsQueues"
        );
        assert!(
            app.world().get::<SkyBfsPending>(orphan).is_none(),
            "cleanup pass must not tag the orphan SkyBfsPending"
        );
    }

    fn build_pull_block_neighbor_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, pull_block_neighbor_edges);
        app
    }

    fn build_pull_sky_neighbor_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, pull_sky_neighbor_edges);
        app
    }

    /// Spawns two single-chunk columns at (0,0) and (1,0), wires the
    /// dimension's `ColumnIndex` so `resolve_neighbor_chunk` finds them,
    /// and returns `(column_a, column_b)`. The caller fills in per-chunk
    /// components.
    fn spawn_two_neighbor_columns(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        chunk_a: bevy_ecs::entity::Entity,
        chunk_b: bevy_ecs::entity::Entity,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let column_a = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        (column_a, column_b)
    }

    #[test]
    fn pull_block_neighbor_edges_pulls_from_loaded_neighbor() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        // Two adjacent columns: column_a at x=0, column_b at x=1, both at z=0.
        // Chunk A in column_a at chunk_pos (0,0,0) with BlockLight Uniform(8).
        // Chunk B in column_b at chunk_pos (1,0,0) with BlockLight Null;
        // when B gets Added<ChunkLoaded>, it should pull face cells from A
        // (A is West of B; from B's frame, light enters via the West face).
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: already loaded, with uniform block light = 8.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(8)),
            BlockParkedEgress::default(),
            BlockInbox::default(),
            ChunkLoaded,
        ));

        // Chunk B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockParkedEgress::default(),
            BlockInbox::default(),
        ));

        // Drain the existing Added<ChunkLoaded> flag for chunk_a by running
        // one tick first with chunk_b not yet ChunkLoaded.
        app.update();

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let inbox = app
            .world()
            .get::<BlockInbox>(chunk_b)
            .expect("inbox on B");
        assert_eq!(
            inbox.0.len(),
            256,
            "B pulls 16x16 face cells from A (block-light)"
        );
        let west_index = Direction::West.index() as u8;
        for w in inbox.0.iter() {
            assert_eq!(w.face(), west_index, "face index is West (entry from A)");
            assert_eq!(w.level(), 7, "level = 8 - 1 manhattan attenuation");
        }
        assert!(
            app.world().get::<BlockBfsPending>(chunk_b).is_some(),
            "B marked BlockBfsPending (pulled face cells into its inbox)"
        );
        // Pure non-mutating face-cell read on A — no state change on A.
        assert!(
            app.world().get::<BlockBfsPending>(chunk_a).is_none(),
            "neighbour A stays clean — non-mutating face-cell pull is not a state change on A"
        );
    }

    #[test]
    fn pull_sky_neighbor_edges_pulls_from_loaded_neighbor() {
        let mut app = build_pull_sky_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        // Mirror of the block-side test on the sky channel.
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: already loaded, with uniform sky light = 8.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            SkyLight(crate::storage::LightStorage::Uniform(8)),
            SkyParkedEgress::default(),
            SkyInbox::default(),
            ChunkLoaded,
        ));

        // Chunk B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            SkyLight::default(),
            SkyParkedEgress::default(),
            SkyInbox::default(),
        ));

        app.update();

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let inbox = app
            .world()
            .get::<SkyInbox>(chunk_b)
            .expect("sky inbox on B");
        assert_eq!(
            inbox.0.len(),
            256,
            "B pulls 16x16 face cells from A (sky-light)"
        );
        let west_index = Direction::West.index() as u8;
        for w in inbox.0.iter() {
            assert_eq!(w.face(), west_index, "face index is West (entry from A)");
            assert_eq!(w.level(), 7, "level = 8 - 1 manhattan attenuation");
        }
        assert!(
            app.world().get::<SkyBfsPending>(chunk_b).is_some(),
            "B marked SkyBfsPending (pulled face cells into its inbox)"
        );
        assert!(
            app.world().get::<SkyBfsPending>(chunk_a).is_none(),
            "neighbour A stays clean — non-mutating face-cell pull is not a state change on A"
        );
    }

    #[test]
    fn pull_block_neighbor_edges_drains_pending_egress_on_load() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // A is West of B. From A's frame, the East face (index 5) points
        // toward B. So A's BlockParkedEgress entry with face=East addresses
        // B; the pull system should drain it.
        let east_index = Direction::East.index() as u8;
        let mut parked = BlockParkedEgress::default();
        parked.0.push(CrossChunkWavefront::new(east_index, 3, 5, 9));

        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight::default(),
            parked,
            BlockInbox::default(),
            ChunkLoaded,
        ));

        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockParkedEgress::default(),
            BlockInbox::default(),
        ));

        // Tick once to consume the initial Added<ChunkLoaded> on A.
        app.update();

        let a_pending_before = app
            .world()
            .get::<BlockParkedEgress>(chunk_a)
            .expect("parked on A");
        assert_eq!(
            a_pending_before.0.len(),
            1,
            "parked entry survives first tick"
        );

        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let a_pending_after = app
            .world()
            .get::<BlockParkedEgress>(chunk_a)
            .expect("parked on A");
        assert!(
            a_pending_after.0.is_empty(),
            "A's parked outbox drained after B loaded"
        );

        let b_incoming = app
            .world()
            .get::<BlockInbox>(chunk_b)
            .expect("inbox on B");
        let west_index = Direction::West.index() as u8;
        let drained = b_incoming
            .0
            .iter()
            .find(|w| w.cell_x() == 3 && w.cell_z() == 5 && w.level() == 9);
        assert!(
            drained.is_some(),
            "drained parked wavefront landed in B's inbox"
        );
        assert_eq!(drained.unwrap().face(), west_index);

        assert!(
            app.world().get::<BlockBfsPending>(chunk_a).is_some(),
            "A marked BlockBfsPending"
        );
    }

    /// Block-channel asymmetry: there is no `Uniform(15)`-neighbour escape
    /// hatch on the block side (Assumption A2 — no block-light fast-path
    /// produces `Uniform(15)` at seed time). Two chunks loading in the same
    /// tick must NOT pull from each other, even if one neighbour happens to
    /// carry a hand-authored `Uniform(15)` block-light value.
    #[test]
    fn pull_block_neighbor_skips_newly_loaded_neighbor() {
        let mut app = build_pull_block_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: hand-authored `Uniform(15)` block light. In production no
        // seed-time fast-path ever produces this for block channel, but the
        // test fixture sets it to confirm the system still skips A because
        // A is `Added<ChunkLoaded>` this tick.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(15)),
            BlockParkedEgress::default(),
            BlockInbox::default(),
        ));

        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockParkedEgress::default(),
            BlockInbox::default(),
        ));

        // Both chunks land in newly_loaded_set in the same tick.
        app.world_mut().entity_mut(chunk_a).insert(ChunkLoaded);
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let b_incoming = app
            .world()
            .get::<BlockInbox>(chunk_b)
            .expect("inbox on B");
        assert!(
            b_incoming.0.is_empty(),
            "B must NOT receive face cells from A — block channel has no Uniform(15) escape hatch"
        );
        assert!(
            app.world().get::<BlockBfsPending>(chunk_b).is_none(),
            "B has no inbox wavefronts, so no BlockBfsPending marker should be inserted"
        );
    }

    fn build_consume_needs_full_reseed_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, consume_needs_full_reseed);
        app
    }

    #[test]
    fn consume_needs_full_reseed_marks_all_loaded_chunks_when_scan_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        // Mint a sky-having dimension so each chunk's `InDimension` lookup
        // resolves to a `HasSkyLight` carrier and the per-channel
        // `SkyNeedsInitialSeed` marker is re-inserted alongside the block one.
        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_b = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_unloaded_slot: Option<bevy_ecs::entity::Entity> = None;
        // Attach a finalized scan so the system treats the heightmap as
        // primed and proceeds with the reseed.
        let mut scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        scan.scan_cursor = -1;
        assert!(scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), chunk_unloaded_slot, Some(chunk_b)]
                        .into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_some(),
            "chunk A re-marked BlockNeedsInitialSeed"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_a).is_some(),
            "chunk A re-marked SkyNeedsInitialSeed (sky-having dim)"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_b).is_some(),
            "chunk B re-marked BlockNeedsInitialSeed"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_b).is_some(),
            "chunk B re-marked SkyNeedsInitialSeed (sky-having dim)"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed removed from column"
        );
    }

    /// Regression: when the column's heightmap scan is not yet finalized
    /// (sentinel reads), `consume_needs_full_reseed` must DROP the reseed
    /// instead of re-marking chunks. Re-marking now would cause
    /// `seed_sky_initial` to read sentinel `min_y` and misclassify cave
    /// chunks as Case A (Uniform(15)). The natural lifecycle in
    /// `prime_heightmaps_on_column_spawn` inserts the per-channel markers
    /// once the scan closes.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_not_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        let chunk_b = app.world_mut().spawn(InDimension(dim)).id();
        // No ColumnHeightmapScan attached → unfinalized.
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, Some(chunk_b)]
                        .into_boxed_slice(),
                },
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_none(),
            "chunk A must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<SkyNeedsInitialSeed>(chunk_a).is_none(),
            "chunk A sky marker must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_b).is_none(),
            "chunk B must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    /// A scan present but still mid-scan (not finalized) must also drop the
    /// reseed. Same rationale as the no-scan case.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_mid_progress() {
        let mut app = build_consume_needs_full_reseed_app();

        let dim = app.world_mut().spawn(HasSkyLight).id();
        let chunk_a = app.world_mut().spawn(InDimension(dim)).id();
        // Mid-scan: cursor still at top of range, no bits closed.
        let scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        assert!(!scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, None].into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<BlockNeedsInitialSeed>(chunk_a).is_none(),
            "chunk must NOT be re-marked when scan is mid-progress"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    #[test]
    fn pull_sky_neighbor_pulls_uniform_15_neighbor_even_when_newly_loaded() {
        // Regression test for the case where a Case-A (Uniform(15)) neighbour
        // and a dark (Case-B) chunk both receive Added<ChunkLoaded> in the
        // same tick. An unconditional skip on newly-loaded neighbours would
        // leave the dark chunk at 0 because A's level-15 face cells never
        // reach it. The escape hatch in `pull_sky_neighbor_edges` lets the
        // pull fire when the neighbour is already settled `Uniform(15)`.
        let mut app = build_pull_sky_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let (column_a, column_b) = spawn_two_neighbor_columns(&mut app, dim, chunk_a, chunk_b);

        // Chunk A: Case A — sky light already at Uniform(15) (the heightmap
        // fast-path outcome from seed_sky_initial, observable here before
        // the pull system runs).
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            SkyLight(crate::storage::LightStorage::Uniform(15)),
            SkyParkedEgress::default(),
            SkyInbox::default(),
        ));

        // Chunk B: Case B — sky light starts at Null (dark), has a
        // SkyInbox buffer for the pull to write into.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            SkyLight::default(),
            SkyParkedEgress::default(),
            SkyInbox::default(),
        ));

        // Insert ChunkLoaded on both in the same tick so both land in
        // newly_loaded_set. The pull system runs once after both inserts.
        app.world_mut().entity_mut(chunk_a).insert(ChunkLoaded);
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let inbox = app
            .world()
            .get::<SkyInbox>(chunk_b)
            .expect("sky inbox on B");
        assert_eq!(
            inbox.0.len(),
            256,
            "B must receive 256 sky-light face-cell entries from A (16x16 at level 14)"
        );
        let west_index = Direction::West.index() as u8;
        for w in inbox.0.iter() {
            assert_eq!(
                w.face(),
                west_index,
                "all entries enter from the West face (A is West of B)"
            );
            assert_eq!(
                w.level(),
                14,
                "level = 15 - 1 manhattan attenuation"
            );
        }
        assert!(
            app.world().get::<SkyBfsPending>(chunk_b).is_some(),
            "B must be marked SkyBfsPending so the BFS converge loop runs"
        );
    }

    // ---- Determinism stress tests for the parallel enqueue systems ----

    // splitmix64: a 64-bit PRNG with a 64-bit state. Inlined here so the
    // determinism tests don't need a dev-dep on `rand`. Same algorithm Java's
    // SplittableRandom seeds from; output is deterministic for a given seed.
    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    // Fisher-Yates shuffle driven by splitmix64. Deterministic for a given seed.
    fn deterministic_shuffle<T>(slice: &mut [T], seed: u64) {
        let mut state = seed;
        let n = slice.len();
        for i in (1..n).rev() {
            let r = (splitmix64(&mut state) % ((i + 1) as u64)) as usize;
            slice.swap(i, r);
        }
    }

    fn sorted_queue(q: &[u64]) -> Vec<u64> {
        let mut v = q.to_vec();
        v.sort_unstable();
        v
    }

    #[test]
    fn enqueue_block_light_dirty_set_is_message_order_independent() {
        // Build 32 events across 8 distinct chunks (4 events per chunk).
        // Each event targets a unique (x,y,z) cell inside its chunk so the
        // per-bucket processing is commutative — the same events shuffled
        // produce the same multiset of queue entries plus the same
        // BlockBfsPending marker set.
        const N_CHUNKS: usize = 8;
        const EVENTS_PER_CHUNK: usize = 4;

        let build_events = |chunks: &[bevy_ecs::entity::Entity]| -> Vec<BlockPlaced> {
            let mut events = Vec::with_capacity(N_CHUNKS * EVENTS_PER_CHUNK);
            for (ci, chunk) in chunks.iter().enumerate() {
                for ei in 0..EVENTS_PER_CHUNK {
                    let x = ((ci * 2 + ei) % 16) as i32;
                    let y = (ei * 3) as i32;
                    let z = ((ci + ei * 5) % 16) as i32;
                    // Alternate AIR <-> TORCH_HI per event so we exercise
                    // both the increase and decrease branches.
                    let (old_state, new_state) = if ei % 2 == 0 {
                        (AIR, TORCH_HI)
                    } else {
                        (TORCH_HI, AIR)
                    };
                    events.push(block_placed(
                        *chunk,
                        BlockPos::new(x, y, z),
                        old_state,
                        new_state,
                    ));
                }
            }
            events
        };

        // Build the prototype event list with placeholder chunk entities so
        // the same logical events can be remapped onto each run's actual
        // chunks. Use a throwaway App to mint stable placeholder entity ids.
        let mut proto_app = App::new();
        let proto_chunks: Vec<bevy_ecs::entity::Entity> = (0..N_CHUNKS)
            .map(|_| proto_app.world_mut().spawn_empty().id())
            .collect();
        let baseline_events = build_events(&proto_chunks);

        // The proto_chunks list is what defines "chunk index N" across runs;
        // captured by reference so every run uses the same proto -> real
        // mapping regardless of event order. (collecting unique chunks from
        // the shuffled stream would re-index per shuffle and defeat the test.)
        let run_with_events = |events: &[BlockPlaced]| -> (
            Vec<(usize, Vec<u64>, Vec<u64>)>,
            std::collections::BTreeSet<usize>,
        ) {
            let mut app = build_app();
            let chunks: Vec<bevy_ecs::entity::Entity> =
                (0..N_CHUNKS).map(|_| spawn_chunk(&mut app)).collect();
            let proto_to_real: std::collections::HashMap<bevy_ecs::entity::Entity, bevy_ecs::entity::Entity> =
                proto_chunks
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, chunks[i]))
                    .collect();

            for placed in events {
                let mut remapped = *placed;
                remapped.chunk = *proto_to_real.get(&placed.chunk).unwrap();
                write_placed(&mut app, remapped);
            }
            app.update();

            let mut per_chunk: Vec<(usize, Vec<u64>, Vec<u64>)> = Vec::with_capacity(N_CHUNKS);
            let mut marker_set: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            for (i, c) in chunks.iter().enumerate() {
                if let Some(ws) = app.world().get::<BlockBfsQueues>(*c) {
                    per_chunk.push((
                        i,
                        sorted_queue(&ws.increase_queue),
                        sorted_queue(&ws.decrease_queue),
                    ));
                }
                if app.world().get::<BlockBfsPending>(*c).is_some() {
                    marker_set.insert(i);
                }
            }
            (per_chunk, marker_set)
        };

        let baseline = run_with_events(&baseline_events);

        // Re-run with at least 4 deterministic shuffles and assert
        // multiset-equivalent per-chunk queue contents plus identical marker
        // sets.
        for seed in 1u64..=6 {
            let mut shuffled = baseline_events.clone();
            deterministic_shuffle(&mut shuffled, seed);
            let actual = run_with_events(&shuffled);
            assert_eq!(
                actual.0, baseline.0,
                "per-chunk sorted queues must match baseline under seed {seed}"
            );
            assert_eq!(
                actual.1, baseline.1,
                "BlockBfsPending marker set must match baseline under seed {seed}"
            );
        }
    }

    #[test]
    fn enqueue_sky_light_dirty_set_is_message_order_independent() {
        // Sky-side determinism stress test. Same shape as the block-side test
        // but every chunk is a topmost-of-column sky chunk so the body's
        // y == 15 && is_topmost path also gets exercised.
        const N_CHUNKS: usize = 8;
        const EVENTS_PER_CHUNK: usize = 4;

        let build_events = |chunks: &[bevy_ecs::entity::Entity]| -> Vec<BlockPlaced> {
            let mut events = Vec::with_capacity(N_CHUNKS * EVENTS_PER_CHUNK);
            for (ci, chunk) in chunks.iter().enumerate() {
                for ei in 0..EVENTS_PER_CHUNK {
                    let x = ((ci * 2 + ei) % 16) as i32;
                    // Mix y values including 15 so the top-face branch trips.
                    let y = match ei % 4 {
                        0 => 0,
                        1 => 7,
                        2 => 12,
                        _ => 15,
                    };
                    let z = ((ci + ei * 5) % 16) as i32;
                    // AIR -> LEAVES trips sky_changed (dampening and flags both
                    // change). LEAVES -> AIR reverses it; combined the per-chunk
                    // bucket exercises both polarities of the predicate.
                    let (old_state, new_state) = if ei % 2 == 0 {
                        (AIR, LEAVES)
                    } else {
                        (LEAVES, AIR)
                    };
                    events.push(block_placed(
                        *chunk,
                        BlockPos::new(x, y, z),
                        old_state,
                        new_state,
                    ));
                }
            }
            events
        };

        let mut proto_app = App::new();
        let proto_chunks: Vec<bevy_ecs::entity::Entity> = (0..N_CHUNKS)
            .map(|_| proto_app.world_mut().spawn_empty().id())
            .collect();
        let baseline_events = build_events(&proto_chunks);

        // Same fix as the block-side test: capture proto_chunks by reference
        // so each run uses an identical proto -> real mapping regardless of
        // event order.
        let run_with_events = |events: &[BlockPlaced]| -> (
            Vec<(usize, Vec<u64>, Vec<u64>)>,
            std::collections::BTreeSet<usize>,
        ) {
            let mut app = build_sky_on_placed_app();
            let chunks: Vec<bevy_ecs::entity::Entity> = (0..N_CHUNKS)
                .map(|_| spawn_sky_chunk_topmost(&mut app))
                .collect();
            let proto_to_real: std::collections::HashMap<bevy_ecs::entity::Entity, bevy_ecs::entity::Entity> =
                proto_chunks
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, chunks[i]))
                    .collect();

            for placed in events {
                let mut remapped = *placed;
                remapped.chunk = *proto_to_real.get(&placed.chunk).unwrap();
                write_placed(&mut app, remapped);
            }
            app.update();

            let mut per_chunk: Vec<(usize, Vec<u64>, Vec<u64>)> = Vec::with_capacity(N_CHUNKS);
            let mut marker_set: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            for (i, c) in chunks.iter().enumerate() {
                if let Some(ws) = app.world().get::<SkyBfsQueues>(*c) {
                    per_chunk.push((
                        i,
                        sorted_queue(&ws.increase_queue),
                        sorted_queue(&ws.decrease_queue),
                    ));
                }
                if app.world().get::<SkyBfsPending>(*c).is_some() {
                    marker_set.insert(i);
                }
            }
            (per_chunk, marker_set)
        };

        let baseline = run_with_events(&baseline_events);

        for seed in 1u64..=6 {
            let mut shuffled = baseline_events.clone();
            deterministic_shuffle(&mut shuffled, seed);
            let actual = run_with_events(&shuffled);
            assert_eq!(
                actual.0, baseline.0,
                "per-chunk sorted sky queues must match baseline under seed {seed}"
            );
            assert_eq!(
                actual.1, baseline.1,
                "SkyBfsPending marker set must match baseline under seed {seed}"
            );
        }
    }
}
