use crate::world::storage::view::WorldViewMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_voxel::SectionPos;
use mcrs_voxel::{BlockPos, BlockStateId};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoxelUpdateSet {
    ApplyChanges,
    NetworkSync,
}

#[derive(Message)]
pub struct BlockSetRequest {
    pub dimension: Entity,
    pub pos: BlockPos,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

/// A block write that one cell of a world hands another: a write into a column of the other
/// cell, or a change of a column of its own for the halo of the other cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellBlockWrite {
    pub pos: BlockPos,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
    pub recursion_left: i16,
}

/// The changes of the halo that the owners sent for this tick. They apply before the requests
/// of the tick, and only change the copy: no neighbour hears of them.
#[derive(Resource, Debug, Default)]
pub struct HaloBlockChanges(pub Vec<HaloBlockChange>);

#[derive(Clone, Copy, Debug)]
pub struct HaloBlockChange {
    pub dimension: Entity,
    pub write: CellBlockWrite,
}

/// Applies the changes of the halo that the owners sent, before any write of the tick. They only
/// change the copy: no neighbour hears of them.
///
/// A change to a section that is still landing stays in `changes` for a later tick: the content
/// that lands can be older than the change. The changes that stay keep their order before the
/// changes that arrive later, so a block always ends at the last change of its owner.
pub fn apply_halo_changes(
    blocks: &mut WorldViewMut,
    writer: &mut MessageWriter<BlockPlaced>,
    changes: &mut Vec<HaloBlockChange>,
) {
    changes.retain(|change| {
        write_block(blocks, writer, change.dimension, change.write) == Written::Landing
    });
}

/// Stores `write` in its section and reports the change, with no neighbour or shape update.
pub fn apply_write(
    blocks: &mut WorldViewMut,
    writer: &mut MessageWriter<BlockPlaced>,
    dimension: Entity,
    write: CellBlockWrite,
) {
    write_block(blocks, writer, dimension, write);
}

/// What became of a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Written {
    Stored,
    /// The section is in the dimension, but its blocks did not land yet.
    Landing,
    Dropped,
}

fn write_block(
    blocks: &mut WorldViewMut,
    writer: &mut MessageWriter<BlockPlaced>,
    dimension: Entity,
    write: CellBlockWrite,
) -> Written {
    let Some(mut view) = blocks.get_mut(dimension) else {
        // Stale dimension Entity. Treated as suspicious because the
        // dimension typically outlives a single FixedUpdate; the
        // most common cause is a bus race between dim despawn and
        // BlockSetRequest delivery.
        tracing::warn!(
            target: "voxel_update",
            ?dimension,
            pos = ?write.pos,
            "a block write names no dimension; dropping",
        );
        return Written::Dropped;
    };
    let chunk_pos = SectionPos::from(write.pos);
    let old_state = match view.set_block(write.pos, write.new_state) {
        Ok(old_state) => old_state,
        Err(why) => {
            if view.section_entity(chunk_pos).is_some() {
                return Written::Landing;
            }
            // A voxel update can arrive for a section that just unloaded;
            // this is expected and not an error.
            tracing::trace!(
                target: "voxel_update",
                %why,
                pos = ?write.pos,
                "a block write found no section; dropping",
            );
            return Written::Dropped;
        }
    };
    if old_state == write.new_state {
        return Written::Stored;
    }
    let Some(chunk) = view.section_entity(chunk_pos) else {
        return Written::Stored;
    };
    writer.write(BlockPlaced {
        chunk,
        chunk_pos,
        block_pos: write.pos,
        old_state,
        new_state: write.new_state,
        flags: write.flags,
    });
    Written::Stored
}

#[derive(Message, Clone, Copy)]
pub struct BlockPlaced {
    pub chunk: Entity,
    pub chunk_pos: SectionPos,
    pub block_pos: BlockPos,
    pub old_state: BlockStateId,
    pub new_state: BlockStateId,
    pub flags: BlockUpdateFlags,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::air_section;
    use crate::world::dimension::DimensionTypeConfig;
    use crate::world::storage::section::SectionIndex;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::SystemState;
    use bevy_ecs::world::World;
    use mcrs_minecraft_volume::BlockVolume;

    type HaloParams<'w, 's> = (WorldViewMut<'w, 's>, MessageWriter<'w, BlockPlaced>);

    fn apply(world: &mut World, changes: &mut Vec<HaloBlockChange>) {
        let mut state: SystemState<HaloParams> = SystemState::new(world);
        let (mut blocks, mut writer) = state.get_mut(world).expect("valid params");
        apply_halo_changes(&mut blocks, &mut writer, changes);
        state.apply(world);
    }

    #[test]
    fn a_halo_change_to_a_landing_section_applies_once_the_section_lands() {
        let mut world = World::new();
        world.init_resource::<Messages<BlockPlaced>>();
        let landing = world.spawn_empty().id();
        let mut index = SectionIndex::new();
        index.insert(SectionPos::new(0, 0, 0), landing);
        let dimension = world.spawn((index, DimensionTypeConfig::new(0, 16))).id();
        let stone = BlockStateId::checked(1, 2).expect("a state past air");
        let pos = BlockPos::new(3, 4, 5);
        let change = |pos: BlockPos| HaloBlockChange {
            dimension,
            write: CellBlockWrite {
                pos,
                new_state: stone,
                flags: BlockUpdateFlags::empty(),
                recursion_left: 0,
            },
        };
        let mut changes = vec![change(pos), change(BlockPos::new(40, 4, 5))];

        apply(&mut world, &mut changes);
        assert_eq!(
            changes.len(),
            1,
            "the change waits for its section, and a change with no section is dropped"
        );

        world.entity_mut(landing).insert(air_section());
        apply(&mut world, &mut changes);
        assert!(
            changes.is_empty(),
            "the change applied once its section landed"
        );
        let mut state: SystemState<WorldViewMut> = SystemState::new(&mut world);
        let mut blocks = state.get_mut(&mut world).expect("valid params");
        let view = blocks.get_mut(dimension).expect("a dimension");
        assert_eq!(view.block(pos), Ok(stone));
    }
}
