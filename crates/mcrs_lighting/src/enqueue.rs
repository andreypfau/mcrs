//! Consumes `MessageReader<BlockPlaced>`, derives the section's intra-cell
//! coord via `rem_euclid(16)` on i32, looks up old/new emission via
//! `Res<BlockLightTable>`, and pushes a decrease and/or increase seed into
//! the section's `BlockLightWorkspace` queues per the emission-diff rule:
//! `old_emission > new_emission` → decrease seed at `old_emission`,
//! `new_emission > 0` → increase seed at `new_emission`. `LightDirty` is
//! inserted via `Commands::entity(placed.chunk).insert(LightDirty)` when at
//! least one seed was pushed.
//!
//! Dampening-only changes (`old_emission == new_emission &&
//! old_dampening != new_dampening`) emit a `tracing::warn!` and skip; the
//! cell will desync until cross-section distribute lands. Missing
//! `BlockLight`/`BlockLightWorkspace` components on the message's
//! `chunk` entity also emit a warning and skip — defensive against any
//! lifecycle-ordering hazard.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Query, Res};
use mcrs_minecraft::world::block_update::BlockPlaced;

use crate::bfs::{pack_bfs_entry, ALL_DIRECTIONS_BITSET};
use crate::components::{BlockLightWorkspace, LightDirty};
use crate::table::BlockLightTable;

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut sections: Query<&mut BlockLightWorkspace>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok(mut workspace) = sections.get_mut(placed.chunk) else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing BlockLightWorkspace; lifecycle ordering hazard"
            );
            continue;
        };

        let old_emission = table.emission_for(placed.old_state);
        let new_emission = table.emission_for(placed.new_state);
        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);

        if old_emission == new_emission && old_dampening != new_dampening {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "dampening-only change not yet handled; light will desync until cross-section distribute lands"
            );
            continue;
        }

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let mut pushed = false;

        if old_emission > new_emission {
            workspace.decrease_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                old_emission,
                ALL_DIRECTIONS_BITSET,
                0,
            ));
            pushed = true;
        }

        if new_emission > 0 {
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                new_emission,
                ALL_DIRECTIONS_BITSET,
                0,
            ));
            pushed = true;
        }

        if pushed {
            commands.entity(placed.chunk).insert(LightDirty);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{
        unpack_bfs_entry_level, unpack_bfs_entry_x, unpack_bfs_entry_y, unpack_bfs_entry_z,
    };
    use crate::components::BlockLight;
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_lighting_table_helpers::*;
    use mcrs_minecraft::world::block::BlockUpdateFlags;
    use mcrs_protocol::BlockStateId;

    mod mcrs_lighting_table_helpers {
        use super::*;
        use crate::table::{flag_bits, BlockLightTable};

        pub const AIR: BlockStateId = BlockStateId(0);
        pub const STONE: BlockStateId = BlockStateId(1);
        pub const TORCH_HI: BlockStateId = BlockStateId(2);
        pub const TORCH_LO: BlockStateId = BlockStateId(3);
        pub const LEAVES: BlockStateId = BlockStateId(4);

        pub fn make_test_table() -> BlockLightTable {
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

            BlockLightTable {
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

    fn spawn_section(app: &mut App) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((BlockLight::default(), BlockLightWorkspace::default()))
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
        let entity = spawn_section(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 5, 9), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1, "one increase seed");
        assert!(
            workspace.decrease_queue.is_empty(),
            "no decrease seed for 0 → 14"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted"
        );
    }

    #[test]
    fn enqueue_decrease_on_emitter_removed() {
        let mut app = build_app();
        let entity = spawn_section(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 8, 8), TORCH_HI, AIR),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1, "one decrease seed");
        assert!(
            workspace.increase_queue.is_empty(),
            "no increase seed for 14 → 0"
        );
        let entry = workspace.decrease_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 8);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 8);
        assert_eq!(unpack_bfs_entry_z(entry), 8);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_both_on_swap() {
        let mut app = build_app();
        let entity = spawn_section(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 2, 3), TORCH_HI, TORCH_LO),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(workspace.increase_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            14,
            "decrease at old emission"
        );
        assert_eq!(
            unpack_bfs_entry_level(workspace.increase_queue[0]),
            7,
            "increase at new emission"
        );
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_no_op_on_zero_zero() {
        let mut app = build_app();
        let entity = spawn_section(&mut app);
        // AIR → STONE: both emission=0, both dampening=0 in the test table, so
        // the dampening-only-change branch does NOT trigger and the system
        // simply records no work.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, STONE),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op"
        );
    }

    #[test]
    fn enqueue_dampening_only_change_warns() {
        let mut app = build_app();
        let entity = spawn_section(&mut app);
        // AIR (emission=0, dampening=0) → LEAVES (emission=0, dampening=1).
        // Pure dampening change; the system warns and skips.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "dampening-only skips increase"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "dampening-only skips decrease"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on dampening-only change"
        );
    }

    #[test]
    fn enqueue_missing_components_warns() {
        let mut app = build_app();
        // Spawn an entity WITHOUT BlockLight/BlockLightWorkspace — emulates
        // a section the lighting lifecycle has not yet attached state to.
        let entity = app.world_mut().spawn(()).id();
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, TORCH_HI),
        );

        app.update();

        assert!(
            app.world().get::<BlockLightWorkspace>(entity).is_none(),
            "entity still has no workspace"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty must NOT be inserted on missing components"
        );
    }

    #[test]
    fn enqueue_uses_rem_euclid_for_negative_coords() {
        let mut app = build_app();
        let entity = spawn_section(&mut app);
        // BlockPos::new(-3, 5, -19) — rem_euclid(16) yields (13, 5, 13).
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(-3, 5, -19), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1);
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 13, "x = -3 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 13, "z = -19 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_level(entry), 14);
    }
}
