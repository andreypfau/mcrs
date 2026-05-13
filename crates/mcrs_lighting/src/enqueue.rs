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
use bevy_ecs::prelude::{Added, Commands, Entity, Query, Res};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_engine::world::column::{InChunkColumn, SectionIndex};
use mcrs_minecraft::world::block_update::BlockPlaced;

use crate::bfs::{
    normal_of, pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL,
};
use crate::components::{BlockLight, BlockLightWorkspace, LightDirty, SkyLight, SkyLightWorkspace};
use crate::table::{flag_bits, BlockLightTable};

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut sections: Query<(&mut BlockLight, &mut BlockLightWorkspace)>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace)) = sections.get_mut(placed.chunk) else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing BlockLight/BlockLightWorkspace; lifecycle ordering hazard"
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
            // The decrease BFS only walks neighbours, so the seed cell itself
            // must be cleared up front; otherwise the source position keeps
            // its previous emitted level after the emitter is removed.
            light.0.set(x as usize, y as usize, z as usize, 0);

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
            // `FLAG_WRITE_LEVEL` makes the BFS write the source cell to
            // `new_emission` before stepping outward, so the source position
            // is established before any neighbour is reached.
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                new_emission,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
            pushed = true;
        }

        if pushed {
            commands.entity(placed.chunk).insert(LightDirty);
        }
    }
}

/// Seeds the top face of every newly-attached topmost-of-column `ChunkSection`
/// with 256 BFS entries at `(x, z, 15)` level `15` carrying `FLAG_WRITE_LEVEL`.
///
/// "Topmost of column" is decided against the column's `SectionIndex`:
/// `chunk_pos.y == min_section_y + sections.len() - 1`. Non-topmost sections
/// (lower in the column) seed nothing — sky light reaches them only via the
/// downward BFS step from the section above. Sections in skyless dimensions
/// never receive a `SkyLight` component (the `SkyLightBundle` insertion gate
/// in `lifecycle::attach_lighting_state` keys on `HasSkyLight`), so the
/// `Added<SkyLight>` filter self-gates this system.
pub fn enqueue_sky_light_initial(
    mut newly_added: Query<
        (Entity, &ChunkPos, &InChunkColumn, &mut SkyLightWorkspace),
        Added<SkyLight>,
    >,
    columns: Query<&SectionIndex>,
    mut commands: Commands,
) {
    for (section_entity, chunk_pos, in_column, mut workspace) in newly_added.iter_mut() {
        let Ok(section_index) = columns.get(in_column.0) else {
            continue;
        };
        let top_chunk_y =
            section_index.min_section_y + section_index.sections.len() as i32 - 1;
        if chunk_pos.y != top_chunk_y {
            continue;
        }

        workspace.increase_queue.reserve(256);
        for z in 0..16u8 {
            for x in 0..16u8 {
                workspace.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_WRITE_LEVEL,
                ));
            }
        }
        commands.entity(section_entity).insert(LightDirty);
    }
}

/// Reacts to `BlockPlaced` by enqueuing sky-light decrease and increase seeds
/// whenever the placed block changes either its dampening or its
/// `PROPAGATES_SKYLIGHT_DOWN` flag. The system also records the world Y of the
/// change into the per-column `block_change_tracker` so the downstream
/// heightmap-update pass can decide whether the column's surface dropped.
///
/// Missing `SkyLight`/`SkyLightWorkspace` components on the target section
/// emit a `tracing::warn!` and skip without panic; this defends against
/// `BlockPlaced` reaching a skyless-dim section (where the bundle is never
/// attached) or arriving before the lighting bundle insertion has flushed.
pub fn enqueue_sky_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut sections: Query<(&mut SkyLight, &mut SkyLightWorkspace)>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((light, mut workspace)) = sections.get_mut(placed.chunk) else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing SkyLight/SkyLightWorkspace; skipping sky enqueue"
            );
            continue;
        };

        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);
        let old_flags = table.flags_for(placed.old_state);
        let new_flags = table.flags_for(placed.new_state);

        let sky_changed = old_dampening != new_dampening
            || (old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN)
                != (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN);
        if !sky_changed {
            continue;
        }

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let stored = light.0.get(x as usize, y as usize, z as usize);
        workspace.decrease_queue.push(pack_bfs_entry(
            x,
            z,
            y,
            stored,
            ALL_DIRECTIONS_BITSET,
            0,
        ));

        if y == 15 {
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                15,
                15,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
        } else {
            for d in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let (dx, dy, dz) = normal_of(d);
                let nx = x as i8 + dx;
                let ny = y as i8 + dy;
                let nz = z as i8 + dz;
                if !(0..16).contains(&nx)
                    || !(0..16).contains(&ny)
                    || !(0..16).contains(&nz)
                {
                    continue;
                }
                let neighbour_level =
                    light.0.get(nx as usize, ny as usize, nz as usize);
                workspace.increase_queue.push(pack_bfs_entry(
                    nx as u8,
                    nz as u8,
                    ny as u8,
                    neighbour_level,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_RECHECK_LEVEL,
                ));
            }
        }

        let column_idx = (z as usize) * 16 + (x as usize);
        let world_y = placed.block_pos.y;
        if world_y > workspace.block_change_tracker[column_idx] {
            workspace.block_change_tracker[column_idx] = world_y;
        }

        commands.entity(placed.chunk).insert(LightDirty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{
        unpack_bfs_entry_flags, unpack_bfs_entry_level, unpack_bfs_entry_x, unpack_bfs_entry_y,
        unpack_bfs_entry_z,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_engine::world::column::{InChunkColumn, SectionIndex};
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

    fn build_sky_initial_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, enqueue_sky_light_initial);
        app
    }

    fn spawn_column_with_sections(
        app: &mut App,
        min_section_y: i32,
        section_slots: Vec<Option<bevy_ecs::entity::Entity>>,
    ) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn(SectionIndex {
                min_section_y,
                sections: section_slots.into_boxed_slice(),
            })
            .id()
    }

    #[test]
    fn enqueue_sky_initial_seeds_topmost_section_only() {
        let mut app = build_sky_initial_app();

        let section = app.world_mut().spawn_empty().id();
        let column = spawn_column_with_sections(&mut app, 0, vec![Some(section)]);
        app.world_mut().entity_mut(section).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(section)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            256,
            "topmost-of-column section seeds 256 entries (16 x 16 at y=15)"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "initial seed does not push decrease entries"
        );
        for entry in &workspace.increase_queue {
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15, "y == 15");
            assert_eq!(unpack_bfs_entry_level(*entry), 15, "level == 15");
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "FLAG_WRITE_LEVEL bit set on every seed"
            );
        }
        assert!(
            app.world().get::<LightDirty>(section).is_some(),
            "LightDirty inserted on topmost-of-column seed"
        );
    }

    #[test]
    fn enqueue_sky_initial_skips_non_topmost() {
        let mut app = build_sky_initial_app();

        let section_below = app.world_mut().spawn_empty().id();
        let section_topmost = app.world_mut().spawn_empty().id();
        // Two-section column: chunk-Y 0 (below) and chunk-Y 1 (topmost).
        let column = spawn_column_with_sections(
            &mut app,
            0,
            vec![Some(section_below), Some(section_topmost)],
        );
        // Only the below section gets SkyLight added; topmost is left bare
        // so this single test does not also seed an unrelated section.
        app.world_mut().entity_mut(section_below).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(section_below)
            .expect("sky workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "non-topmost section seeds nothing"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "non-topmost section seeds no decrease"
        );
        assert!(
            app.world().get::<LightDirty>(section_below).is_none(),
            "LightDirty NOT inserted on non-topmost-of-column section"
        );
    }

    fn build_sky_on_placed_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        app
    }

    fn spawn_sky_section(app: &mut App) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((SkyLight::default(), SkyLightWorkspace::default()))
            .id()
    }

    #[test]
    fn enqueue_sky_on_block_placed_writes_tracker() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section(&mut app);
        // AIR (damp=0, propagates) -> LEAVES (damp=1, no propagates flag);
        // sky_changed predicate trips on both dampening and flag delta.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 10, 8), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        // column_idx = z * 16 + x = 8 * 16 + 8 = 136; world_y = 10.
        assert_eq!(
            workspace.block_change_tracker[136], 10,
            "block_change_tracker[z*16+x] holds the world Y of the change"
        );
        assert!(
            !workspace.decrease_queue.is_empty(),
            "dampening change pushes a decrease seed"
        );
        assert!(
            !workspace.increase_queue.is_empty(),
            "y=10 (non-top) pushes neighbour-support increase seeds"
        );
        // y=10 (intra-section, not 15) -> six neighbour seeds.
        assert_eq!(
            workspace.increase_queue.len(),
            6,
            "y < 15 produces exactly six neighbour-support seeds"
        );
        for entry in &workspace.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
        }
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted after dampening change"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_writes_tracker_keeps_max() {
        // Two BlockPlaced events at the same (x, z) column; tracker must keep
        // the larger world Y to preserve the highest changed cell.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 12, 8), AIR, LEAVES),
        );
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.block_change_tracker[136], 12,
            "tracker keeps max(existing, world_y); 12 > 5 wins"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_top_seeds_top_face() {
        // y == 15 path: a single top-face increase seed instead of six
        // neighbour seeds.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            1,
            "y == 15 produces exactly one top-face seed"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-of-section seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_when_predicate_false() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section(&mut app);
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

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            workspace.block_change_tracker.iter().all(|&v| v == 0),
            "tracker untouched when predicate is false"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op sky enqueue"
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
            // Section without SkyLight/SkyLightWorkspace (skyless-dim shape).
            let entity = app
                .world_mut()
                .spawn((BlockLight::default(), BlockLightWorkspace::default()))
                .id();
            write_placed(
                &mut app,
                block_placed(entity, BlockPos::new(2, 3, 4), AIR, LEAVES),
            );

            app.update();

            assert!(
                app.world().get::<SkyLightWorkspace>(entity).is_none(),
                "entity still has no sky workspace"
            );
            assert!(
                app.world().get::<LightDirty>(entity).is_none(),
                "LightDirty must NOT be inserted when SkyLight is missing"
            );
        });

        let bytes = captured.lock().unwrap();
        let output = String::from_utf8_lossy(&bytes);
        assert!(
            output.contains("BlockPlaced.chunk missing SkyLight/SkyLightWorkspace"),
            "expected warn substring in captured tracing output, got: {output}"
        );
    }
}
