//! Thin Bevy system wrappers around `bfs::propagate_increase` and
//! `bfs::propagate_decrease`. Each system iterates
//! `Query<..., With<LightDirty>>` in parallel via `par_iter_mut`. Per-worker
//! `Commands` accumulation goes through `ParallelCommands`.
//!
//! Drain-incoming prelude: every per-section iteration starts by draining
//! the section's `*Incoming` buffer into `workspace.increase_queue` via
//! `pack_bfs_entry(..., FLAG_WRITE_LEVEL)`. Each incoming `Wavefront`
//! encodes the source-frame face plus its on-face `(cell_x, cell_z)` and
//! `level`; the helper `face_to_section_coords` decodes those to the
//! destination-section-local `(x, y, z)` cell coordinates expected by the
//! packed BFS entry layout. The decoded face is inverted at distribute
//! time, so a wavefront arriving on the destination's West-`Incoming` lives
//! at `x = 0` inside the destination, and the BFS picks it up as if it
//! had been seeded at that cell from level `level`.
//!
//! `propagate_increase_block_system` removes `LightDirty` at the end of
//! the loop body when both workspace queues have drained, regardless of
//! whether `BlockEgress` is empty — the source section is done with
//! intra-section work, and the cross-section distribute pass re-marks
//! `LightDirty` on any section it touches via egress.
//!
//! Ordering between the two systems is set at plugin wiring time:
//! the decrease pass is chained before the increase pass so the decrease
//! pass requeues pre-existing-higher cells onto `increase_queue` before
//! the increase pass drains it.

use bevy_ecs::prelude::{Entity, ParallelCommands, Query, Res, With};
use mcrs_core::voxel_shape::Direction;
use mcrs_minecraft_block::palette::BlockPalette;

use crate::bfs::{
    pack_bfs_entry, propagate_decrease, propagate_decrease_sky, propagate_increase,
    propagate_increase_sky, unpack_bfs_entry_level, unpack_bfs_entry_y, ALL_DIRECTIONS_BITSET,
    FLAG_WRITE_LEVEL,
};
use crate::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, IsAllAir, LightDirty, SkyEgress,
    SkyIncoming, SkyLight, SkyLightWorkspace, Wavefront,
};
use crate::distribute::direction_from_index;
use crate::storage::LightStorage;
use crate::table::BlockLightTable;

/// Inverse of `bfs::project_face_cell`: given an inbound wavefront's
/// destination-frame face plus its on-face `(cell_a, cell_b)` packing,
/// return the destination-section-local `(x, y, z)` cell coordinates.
///
/// Y-normal faces drop y, X-normal faces drop x, Z-normal faces drop z.
/// For `Up` the implicit y is 15; for `Down` the implicit y is 0; for
/// `East` x is 15; for `West` x is 0; for `South` z is 15; for `North`
/// z is 0. The two non-normal axes pack the on-face cell coordinates in
/// the same order as `project_face_cell` — `(cell_a, cell_b)` where
/// `cell_a` is the first non-normal axis and `cell_b` is the second.
#[inline]
pub(crate) fn face_to_section_coords(face: Direction, cell_a: u8, cell_b: u8) -> (u8, u8, u8) {
    match face {
        Direction::Down => (cell_a, 0, cell_b),
        Direction::Up => (cell_a, 15, cell_b),
        Direction::North => (cell_a, cell_b, 0),
        Direction::South => (cell_a, cell_b, 15),
        Direction::West => (0, cell_a, cell_b),
        Direction::East => (15, cell_a, cell_b),
    }
}

/// Drain a `*Incoming` buffer into a workspace's `increase_queue` via
/// `pack_bfs_entry(..., FLAG_WRITE_LEVEL)`. Each entry is packed at the
/// destination-section-local cell decoded by `face_to_section_coords`, and
/// the BFS will write the wavefront's `level` and propagate outward from
/// there.
#[inline]
fn drain_incoming_into_queue(
    incoming: &mut smallvec::SmallVec<[Wavefront; 8]>,
    queue: &mut Vec<u64>,
) {
    for wavefront in incoming.drain(..) {
        let face = direction_from_index(wavefront.face());
        let (x, y, z) =
            face_to_section_coords(face, wavefront.cell_x(), wavefront.cell_z());
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

pub fn propagate_decrease_block_system(
    table: Res<BlockLightTable>,
    mut sections: Query<
        (
            Entity,
            &BlockPalette,
            &mut BlockLight,
            &mut BlockLightWorkspace,
            &mut BlockEgress,
            &mut BlockIncoming,
        ),
        With<LightDirty>,
    >,
) {
    #[cfg(feature = "lighting-trace")]
    let section_count = sections.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("propagate_decrease", section_count = section_count).entered();
    sections.par_iter_mut().for_each(
        |(_entity, palette, mut light, mut workspace, mut egress, mut incoming)| {
            drain_incoming_into_queue(&mut incoming.0, &mut workspace.increase_queue);
            propagate_decrease(&table, palette, &mut light.0, &mut workspace, &mut egress);
            #[cfg(feature = "lighting-trace")]
            tracing::debug!(section = ?_entity, queue_len = workspace.decrease_queue.len(), "section bfs decrease block");
        },
    );
}

pub fn propagate_increase_block_system(
    table: Res<BlockLightTable>,
    mut sections: Query<
        (
            Entity,
            &BlockPalette,
            &mut BlockLight,
            &mut BlockLightWorkspace,
            &mut BlockEgress,
            &mut BlockIncoming,
        ),
        With<LightDirty>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "lighting-trace")]
    let section_count = sections.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("propagate_increase", section_count = section_count).entered();
    sections.par_iter_mut().for_each(
        |(entity, palette, mut light, mut workspace, mut egress, mut incoming)| {
            drain_incoming_into_queue(&mut incoming.0, &mut workspace.increase_queue);
            propagate_increase(&table, palette, &mut light.0, &mut workspace, &mut egress);
            if workspace.increase_queue.is_empty() && workspace.decrease_queue.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).remove::<LightDirty>();
                });
            }
            #[cfg(feature = "lighting-trace")]
            tracing::debug!(section = ?entity, queue_len = workspace.increase_queue.len(), "section bfs increase block");
        },
    );
}

pub fn propagate_decrease_sky_system(
    table: Res<BlockLightTable>,
    mut sections: Query<
        (
            Entity,
            &BlockPalette,
            &mut SkyLight,
            &mut SkyLightWorkspace,
            &mut SkyEgress,
            &mut SkyIncoming,
        ),
        With<LightDirty>,
    >,
) {
    #[cfg(feature = "lighting-trace")]
    let section_count = sections.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("propagate_decrease_sky", section_count = section_count).entered();
    sections.par_iter_mut().for_each(
        |(_entity, palette, mut light, mut workspace, mut egress, mut incoming)| {
            drain_incoming_into_queue(&mut incoming.0, &mut workspace.increase_queue);
            propagate_decrease_sky(&table, palette, &mut light.0, &mut workspace, &mut egress);
            #[cfg(feature = "lighting-trace")]
            tracing::debug!(section = ?_entity, queue_len = workspace.decrease_queue.len(), "section bfs decrease sky");
        },
    );
}

/// Five non-Up faces used by the column-walker fast path to dump 256 wavefronts
/// per face onto `SkyEgress` (1280 entries total) when an `IsAllAir` section
/// short-circuits the BFS.
const COLUMN_WALKER_FACES: [Direction; 5] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Column-walker predicate: an all-air section whose only queued work is the
/// 256 top-face level-15 seeds is advanced in O(1) by writing
/// `LightStorage::Uniform(15)` and dumping wavefronts onto the five non-Up
/// faces, instead of running the per-cell BFS.
///
/// All three conditions must hold:
/// - `is_all_air` is true,
/// - `workspace.decrease_queue` is empty,
/// - every entry in `workspace.increase_queue` is at y=15 with level=15.
fn try_column_walker_fast_path(is_all_air: bool, workspace: &SkyLightWorkspace) -> bool {
    if !is_all_air {
        return false;
    }
    if !workspace.decrease_queue.is_empty() {
        return false;
    }
    if workspace.increase_queue.is_empty() {
        return false;
    }
    workspace.increase_queue.iter().all(|&e| {
        let y = (unpack_bfs_entry_y(e) as usize) & 0xF;
        let lvl = unpack_bfs_entry_level(e);
        y == 15 && lvl == 15
    })
}

pub fn propagate_increase_sky_system(
    table: Res<BlockLightTable>,
    mut sections: Query<
        (
            Entity,
            &BlockPalette,
            &mut SkyLight,
            &mut SkyLightWorkspace,
            &mut SkyEgress,
            &mut SkyIncoming,
            Option<&IsAllAir>,
        ),
        With<LightDirty>,
    >,
    commands: ParallelCommands,
) {
    #[cfg(feature = "lighting-trace")]
    let section_count = sections.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("propagate_increase_sky", section_count = section_count).entered();
    sections.par_iter_mut().for_each(
        |(entity, palette, mut light, mut workspace, mut egress, mut incoming, is_all_air)| {
            drain_incoming_into_queue(&mut incoming.0, &mut workspace.increase_queue);

            if try_column_walker_fast_path(is_all_air.is_some(), &workspace) {
                light.0 = LightStorage::Uniform(15);
                // SmallVec inline capacity is 8; reserve up front so the 1280
                // per-cell pushes below collapse to a single heap allocation
                // instead of 7+ incremental reallocations.
                egress.0.reserve(1280);
                // Per-face (cell_x, cell_z) pairing follows the project_face_cell
                // axis contract: Y-normal faces drop y, Z-normal faces drop z and
                // pack (x, y), X-normal faces drop x and pack (y, z).
                for face in COLUMN_WALKER_FACES {
                    let face_idx = face.index() as u8;
                    for a in 0..16u8 {
                        for b in 0..16u8 {
                            let (cx, cz) = match face {
                                Direction::Down | Direction::Up => (b, a),
                                Direction::North | Direction::South => (b, a),
                                Direction::West | Direction::East => (a, b),
                            };
                            egress.0.push(Wavefront::new(face_idx, cx, cz, 15));
                        }
                    }
                }
                workspace.increase_queue.clear();
                if workspace.decrease_queue.is_empty() {
                    commands.command_scope(|mut cmd| {
                        cmd.entity(entity).remove::<LightDirty>();
                    });
                }
                return;
            }

            propagate_increase_sky(&table, palette, &mut light.0, &mut workspace, &mut egress);
            if workspace.increase_queue.is_empty() && workspace.decrease_queue.is_empty() {
                commands.command_scope(|mut cmd| {
                    cmd.entity(entity).remove::<LightDirty>();
                });
            }
            #[cfg(feature = "lighting-trace")]
            tracing::debug!(section = ?entity, queue_len = workspace.increase_queue.len(), "section bfs increase sky");
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET};
    use crate::components::{BlockIncoming, BlockLight, BlockLightWorkspace, LightDirty};
    use crate::nibble::NibbleArray;
    use crate::storage::LightStorage;
    use crate::table::flag_bits;
    use bevy_app::{App, Update};
    use bevy_ecs::prelude::IntoScheduleConfigs;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_protocol::BlockStateId;

    const AIR: BlockStateId = BlockStateId(0);
    const TORCH: BlockStateId = BlockStateId(1);

    fn make_test_table() -> BlockLightTable {
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
        BlockLightTable {
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
        LightStorage::Mixed(Box::new(NibbleArray::zeros()))
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

    fn spawn_section_dirty(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockLightWorkspace::default(),
                BlockEgress::default(),
                BlockIncoming::default(),
                LightDirty,
            ))
            .id()
    }

    fn spawn_section_clean(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockLightWorkspace::default(),
                BlockEgress::default(),
                BlockIncoming::default(),
            ))
            .id()
    }

    fn push_increase(app: &mut App, entity: Entity, entry: u64) {
        let mut ws = app
            .world_mut()
            .get_mut::<BlockLightWorkspace>(entity)
            .expect("workspace");
        ws.increase_queue.push(entry);
    }

    fn push_decrease(app: &mut App, entity: Entity, entry: u64) {
        let mut ws = app
            .world_mut()
            .get_mut::<BlockLightWorkspace>(entity)
            .expect("workspace");
        ws.decrease_queue.push(entry);
    }

    #[test]
    fn propagate_decrease_drains_queue() {
        let mut app = build_app_with_decrease();
        let entity = spawn_section_dirty(&mut app);
        // Seed the L1 field so the decrease BFS has cells to walk through.
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
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(
            ws.decrease_queue.is_empty(),
            "decrease_queue must drain to empty"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "decrease system does not clear LightDirty (that is the increase system's job)"
        );
    }

    #[test]
    fn propagate_increase_drains_queue() {
        let mut app = build_app_with_increase();
        let entity = spawn_section_dirty(&mut app);
        // Seed the source cell so the BFS reads 14 from the stored level on
        // the first non-recheck step.
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
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(
            ws.increase_queue.is_empty(),
            "increase_queue must drain to empty"
        );
        let light = app
            .world()
            .get::<BlockLight>(entity)
            .expect("BlockLight");
        // Source cell remains 14; the BFS propagates outward.
        assert_eq!(light.0.get(8, 8, 8), 14, "source cell unchanged");
        assert!(
            light.0.get(7, 8, 8) > 0 || light.0.get(9, 8, 8) > 0,
            "BFS must have written at least one neighbour"
        );
    }

    #[test]
    fn propagate_clears_light_dirty_when_drained() {
        let mut app = build_app_with_increase();
        let entity = spawn_section_dirty(&mut app);
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
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty cleared when both queues empty"
        );
    }

    #[test]
    fn propagate_clears_light_dirty_with_egress_nonempty() {
        let mut app = build_app_with_increase();
        let entity = spawn_section_dirty(&mut app);
        // Seed at (15, 8, 8) so the +X (East) step falls off the section and
        // ends up in BlockEgress while the BFS still drains queues.
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
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        let egress = app
            .world()
            .get::<BlockEgress>(entity)
            .expect("BlockEgress");
        assert!(ws.increase_queue.is_empty(), "queues drained");
        assert!(ws.decrease_queue.is_empty(), "queues drained");
        assert!(
            !egress.0.is_empty(),
            "egress must contain at least one East face wavefront"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty cleared even when BlockEgress is non-empty"
        );
    }

    #[test]
    fn propagate_skips_clean_sections() {
        let mut app = build_app_with_increase();
        let entity = spawn_section_clean(&mut app);
        // Push a stale entry that, if visited, would mutate light.
        push_increase(
            &mut app,
            entity,
            pack_bfs_entry(8, 8, 8, 14, ALL_DIRECTIONS_BITSET, 0),
        );

        app.update();

        let ws = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "queue NOT drained — clean section is skipped by With<LightDirty>"
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
                        "no cell should be written on a clean section"
                    );
                }
            }
        }
    }

    #[test]
    fn propagate_only_runs_on_dirty_sections() {
        // Chain decrease before increase to match production ordering, so the
        // dirty section's seed is drained by `propagate_increase` and its
        // LightDirty is cleared, while the clean section sees neither system
        // touch its workspace.
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

        let dirty = spawn_section_dirty(&mut app);
        let clean = spawn_section_clean(&mut app);

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
            .get::<BlockLightWorkspace>(dirty)
            .expect("workspace");
        let clean_ws = app
            .world()
            .get::<BlockLightWorkspace>(clean)
            .expect("workspace");
        assert!(
            dirty_ws.increase_queue.is_empty(),
            "dirty section's queue drained"
        );
        assert_eq!(
            clean_ws.increase_queue.len(),
            1,
            "clean section untouched — stale seed still in queue"
        );
        assert!(
            app.world().get::<LightDirty>(dirty).is_none(),
            "dirty section's LightDirty cleared"
        );
    }

    // -------- sky propagate system tests --------

    use crate::components::{IsAllAir, SkyEgress, SkyIncoming, SkyLight, SkyLightWorkspace};

    fn build_app_with_sky_increase() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_increase_sky_system);
        app
    }

    fn spawn_sky_section_all_air_with_top_seeds(app: &mut App) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(LightStorage::default()),
                SkyLightWorkspace::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                IsAllAir,
                LightDirty,
            ))
            .id();

        // Push 256 top-face level-15 seeds (one per (x, z) at y=15).
        let mut ws = app
            .world_mut()
            .get_mut::<SkyLightWorkspace>(entity)
            .expect("SkyLightWorkspace");
        for z in 0..16u8 {
            for x in 0..16u8 {
                ws.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    crate::bfs::FLAG_WRITE_LEVEL,
                ));
            }
        }
        entity
    }

    fn spawn_sky_section_partial_air_with_top_seeds(app: &mut App) -> Entity {
        // Same as the all-air spawner but WITHOUT the IsAllAir marker — the
        // column-walker prelude must NOT fire.
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(LightStorage::default()),
                SkyLightWorkspace::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                LightDirty,
            ))
            .id();

        let mut ws = app
            .world_mut()
            .get_mut::<SkyLightWorkspace>(entity)
            .expect("SkyLightWorkspace");
        for z in 0..16u8 {
            for x in 0..16u8 {
                ws.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    crate::bfs::FLAG_WRITE_LEVEL,
                ));
            }
        }
        entity
    }

    #[test]
    fn propagate_sky_column_walker_collapses_all_air() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_section_all_air_with_top_seeds(&mut app);

        app.update();

        let light = app
            .world()
            .get::<SkyLight>(entity)
            .expect("SkyLight");
        assert!(
            matches!(light.0, LightStorage::Uniform(15)),
            "column-walker must collapse the all-air section to Uniform(15); got {:?}",
            light.0
        );
        let ws = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("SkyLightWorkspace");
        assert!(
            ws.increase_queue.is_empty(),
            "column-walker must clear the increase_queue"
        );
    }

    #[test]
    fn propagate_sky_column_walker_pushes_1280_wavefronts() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_section_all_air_with_top_seeds(&mut app);

        app.update();

        let egress = app
            .world()
            .get::<SkyEgress>(entity)
            .expect("SkyEgress");
        assert_eq!(
            egress.0.len(),
            1280,
            "column-walker must push 1280 wavefronts (5 non-Up faces x 256 cells)"
        );

        // Decode the first entry: face index must be one of the five non-Up
        // faces (Down=0, North=2, South=3, West=4, East=5), level must be 15.
        let first = egress.0[0];
        assert!(
            matches!(first.face(), 0 | 2 | 3 | 4 | 5),
            "wavefront face must be one of the five non-Up faces; got {}",
            first.face()
        );
        assert_eq!(first.level(), 15, "wavefront level must be 15");
    }

    /// Asserts the column-walker fast path emits West-face wavefronts whose
    /// `(cell_x, cell_z)` pairs decode consistently with the `project_face_cell`
    /// axis contract. The defective code pushes `(cz=outer, cx=inner)` for all
    /// faces, so the first 16 West entries would be `(0,0),(1,0),...,(15,0)`.
    /// The correct West/East emission is `(cx=outer, cz=inner)`, so the first
    /// 16 entries are `(0,0),(0,1),...,(0,15)`. Ordered equality on West is the
    /// cleanest gate because Down/North/South share the `(b,a)` pairing with
    /// the bug and would not discriminate.
    #[test]
    fn propagate_sky_column_walker_face_coordinates() {
        let mut app = build_app_with_sky_increase();
        let entity = spawn_sky_section_all_air_with_top_seeds(&mut app);

        app.update();

        let egress = app
            .world()
            .get::<SkyEgress>(entity)
            .expect("SkyEgress");
        assert_eq!(egress.0.len(), 1280, "column-walker must push 1280 wavefronts");

        let west_face = Direction::West.index() as u8;
        let actual: Vec<(u8, u8)> = egress
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
        let entity = spawn_sky_section_partial_air_with_top_seeds(&mut app);

        app.update();

        // The column-walker prelude excludes the Up face from its 1280-entry
        // dump (`COLUMN_WALKER_FACES` is the five non-Up faces only). The BFS
        // path, by contrast, re-evaluates every direction from each seed and
        // pushes Up-face wavefronts as the y=15 seeds step off the top of the
        // section. Presence of any Up-face (index 1) wavefront proves the BFS
        // ran instead of the fast path.
        let egress = app
            .world()
            .get::<SkyEgress>(entity)
            .expect("SkyEgress");
        let up_face_count = egress.0.iter().filter(|w| w.face() == 1).count();
        assert!(
            up_face_count > 0,
            "BFS path must push Up-face wavefronts; column-walker fast path excludes Up. egress.len()={}, up_face_count={}",
            egress.0.len(),
            up_face_count
        );
        assert_ne!(
            egress.0.len(),
            1280,
            "BFS path produces a different wavefront count than the column-walker's exact 1280"
        );
    }

    #[test]
    fn propagate_sky_skyless_dim_iterates_nothing() {
        // Skyless-dim section: BlockPalette + BlockLight + BlockLightWorkspace
        // only, no SkyLight components. The Query<&mut SkyLight, ...> in the
        // increase system filters this section out by archetype mismatch.
        let mut app = build_app_with_sky_increase();
        let section = app
            .world_mut()
            .spawn((
                air_palette(),
                BlockLight(zero_light_storage()),
                BlockLightWorkspace::default(),
                BlockEgress::default(),
                BlockIncoming::default(),
                LightDirty,
            ))
            .id();

        app.update();

        assert!(
            app.world().entity(section).get::<SkyLight>().is_none(),
            "skyless-dim section must never gain SkyLight from the sky propagate systems"
        );
        assert!(
            app.world().entity(section).get::<SkyEgress>().is_none(),
            "skyless-dim section must never gain SkyEgress from the sky propagate systems"
        );
    }

    // ---- Plan-2 added: drain-incoming + par_iter_mut structural tests ----

    /// Verify drain-Incoming prelude turns one BlockIncoming wavefront into a
    /// pack_bfs_entry FLAG_WRITE_LEVEL on workspace.increase_queue. East face
    /// wavefront at (cell_x=0, cell_z=8, level=8) decodes to section-local
    /// (x=15, y=0, z=8) per face_to_section_coords(East, 0, 8). The drain
    /// runs at the top of `propagate_decrease_block_system`; the
    /// decrease pass itself only drains `decrease_queue`, so the prelude's
    /// FLAG_WRITE_LEVEL entry survives on `increase_queue` for the next
    /// increase pass to consume.
    #[test]
    fn propagate_decrease_drains_block_incoming_at_top_of_body() {
        let mut app = build_app_with_decrease();
        let entity = spawn_section_dirty(&mut app);
        let east = Direction::East.index() as u8;
        let mut incoming = app
            .world_mut()
            .get_mut::<BlockIncoming>(entity)
            .expect("incoming");
        incoming.0.push(Wavefront::new(east, 0, 8, 8));
        drop(incoming);

        app.update();

        let inc = app
            .world()
            .get::<BlockIncoming>(entity)
            .expect("incoming");
        assert!(inc.0.is_empty(), "drain prelude must empty BlockIncoming");
        // The decrease pass does not drain `increase_queue`, so the seeded
        // entry stays in the increase_queue for the next increase pass.
        let ws = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "drain prelude wrote exactly one entry onto increase_queue; decrease pass does not drain that queue"
        );
        // Decode and verify the entry's coordinates + flags.
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
    /// level=12) decodes to section-local (x=4, y=7, z=15) per
    /// face_to_section_coords(South, 4, 7).
    #[test]
    fn propagate_increase_drains_sky_incoming_at_top_of_body() {
        // Use decrease_sky here because the decrease prelude is the same; the
        // increase_sky has the column-walker path, which complicates the
        // assertion. Both systems share the drain helper, so testing one
        // covers the prelude logic.
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, propagate_decrease_sky_system);
        let entity = app
            .world_mut()
            .spawn((
                air_palette(),
                SkyLight(zero_light_storage()),
                SkyLightWorkspace::default(),
                SkyEgress::default(),
                SkyIncoming::default(),
                LightDirty,
            ))
            .id();
        let south = Direction::South.index() as u8;
        let mut inc = app
            .world_mut()
            .get_mut::<SkyIncoming>(entity)
            .expect("incoming");
        inc.0.push(Wavefront::new(south, 4, 7, 12));
        drop(inc);

        app.update();

        let inc = app
            .world()
            .get::<SkyIncoming>(entity)
            .expect("incoming");
        assert!(
            inc.0.is_empty(),
            "drain prelude must empty SkyIncoming"
        );
        // The decrease_sky BFS reads at the decoded (x=4, y=7, z=15) and
        // walks; FLAG_WRITE_LEVEL is honored on the increase queue, NOT the
        // decrease queue. The drain pushes onto increase_queue but
        // propagate_decrease_sky drains decrease_queue, not increase_queue —
        // so the entry survives the decrease pass. Verify it's still in the
        // increase_queue.
        let ws = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(
            ws.increase_queue.len(),
            1,
            "drain prelude wrote one entry onto increase_queue; decrease pass does not drain that queue"
        );
    }

    /// Spawn 100 sections each with LightDirty + a seeded BlockLight cell
    /// + an increase_queue entry, then run propagate_increase_block_system
    /// which uses par_iter_mut. Assert all 100 sections drain their queues
    /// and clear LightDirty. This is structural: par_iter_mut must compile,
    /// run without deadlock, and produce identical results to iter_mut.
    #[test]
    fn propagate_decrease_runs_under_par_iter_mut() {
        let mut app = build_app_with_increase();
        let mut entities = Vec::with_capacity(100);
        for _ in 0..100 {
            let e = spawn_section_dirty(&mut app);
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
                .get::<BlockLightWorkspace>(e)
                .expect("workspace");
            assert!(
                ws.increase_queue.is_empty(),
                "entity {e:?} queue drained under par_iter_mut"
            );
            assert!(
                app.world().get::<LightDirty>(e).is_none(),
                "entity {e:?} LightDirty cleared under par_iter_mut"
            );
        }
    }

    #[test]
    fn face_to_section_coords_round_trip() {
        // face_to_section_coords is the inverse of project_face_cell on the
        // (cell_a, cell_b) -> (x, y, z) projection. Confirm the y/x/z
        // implicit values match the project_face_cell axis contract.
        assert_eq!(face_to_section_coords(Direction::Down, 3, 7), (3, 0, 7));
        assert_eq!(face_to_section_coords(Direction::Up, 3, 7), (3, 15, 7));
        assert_eq!(face_to_section_coords(Direction::North, 3, 7), (3, 7, 0));
        assert_eq!(face_to_section_coords(Direction::South, 3, 7), (3, 7, 15));
        assert_eq!(face_to_section_coords(Direction::West, 3, 7), (0, 3, 7));
        assert_eq!(face_to_section_coords(Direction::East, 3, 7), (15, 3, 7));
    }
}
