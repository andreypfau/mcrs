//! Thin Bevy system wrappers around `bfs::propagate_increase` and
//! `bfs::propagate_decrease`. Each system iterates
//! `Query<..., With<LightDirty>>` sequentially via `iter_mut`; the parallel
//! upgrade lives behind a future `LightConvergeSchedule` sub-schedule.
//!
//! `propagate_increase_block_system` removes `LightDirty` at the end of
//! the loop body when both workspace queues have drained, regardless of
//! whether `BlockEgress` is empty — the source section is done with
//! intra-section work, and the cross-section distribute pass (when it
//! lands) will re-mark `LightDirty` on any section it touches via egress.
//!
//! Ordering between the two systems is set at plugin wiring time:
//! `LightingSet::PropagateDecrease` is chained before
//! `LightingSet::PropagateIncrease` so the decrease pass requeues
//! pre-existing-higher cells onto `increase_queue` before the increase
//! pass drains it.

use bevy_ecs::prelude::{Commands, Entity, Query, Res, With};
use mcrs_minecraft::world::palette::BlockPalette;

use crate::bfs::{propagate_decrease, propagate_increase};
use crate::components::{BlockEgress, BlockLight, BlockLightWorkspace, LightDirty};
use crate::table::BlockLightTable;

pub fn propagate_decrease_block_system(
    table: Res<BlockLightTable>,
    mut sections: Query<
        (
            &BlockPalette,
            &mut BlockLight,
            &mut BlockLightWorkspace,
            &mut BlockEgress,
        ),
        With<LightDirty>,
    >,
) {
    for (palette, mut light, mut workspace, mut egress) in sections.iter_mut() {
        propagate_decrease(&table, palette, &mut light.0, &mut workspace, &mut egress);
    }
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
        ),
        With<LightDirty>,
    >,
    mut commands: Commands,
) {
    for (entity, palette, mut light, mut workspace, mut egress) in sections.iter_mut() {
        propagate_increase(&table, palette, &mut light.0, &mut workspace, &mut egress);
        if workspace.increase_queue.is_empty() && workspace.decrease_queue.is_empty() {
            commands.entity(entity).remove::<LightDirty>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET};
    use crate::components::{BlockLight, BlockLightWorkspace, LightDirty};
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
}
