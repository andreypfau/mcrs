use crate::block::BlockUpdateFlags;
use bevy_ecs::entity::Entity;
use mcrs_engine::voxel_update::{
    VoxelPlaced, VoxelSetRequest, VoxelUpdateFlags, VoxelUpdatePlugin,
};
use mcrs_protocol::BlockStateId;
use mcrs_voxel_math::BlockPos;

impl VoxelUpdateFlags for BlockUpdateFlags {
    fn notifies_clients(&self) -> bool {
        self.contains(BlockUpdateFlags::CLIENTS)
    }
}

pub type BlockSetRequest = VoxelSetRequest<BlockUpdateFlags>;
pub type BlockPlaced = VoxelPlaced<BlockUpdateFlags>;
pub type BlockUpdatePlugin = VoxelUpdatePlugin<BlockUpdateFlags>;

pub fn remove_block<P: Into<BlockPos>>(dimension: Entity, pos: P) -> BlockSetRequest {
    BlockSetRequest {
        dimension,
        pos: pos.into(),
        new_state: BlockStateId(0).into(),
        flags: BlockUpdateFlags::all(),
        recursion_left: 512,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::IntoScheduleConfigs;
    use mcrs_engine::voxel_update::{VoxelUpdateSet, apply_voxel_set_requests};
    use mcrs_voxel_math::ChunkPos;

    #[test]
    fn set_configured_compile_test() {
        let _ = apply_voxel_set_requests::<BlockUpdateFlags>.in_set(VoxelUpdateSet::ApplyChanges);
    }

    #[test]
    fn block_placed_fields_pub_compile_test() {
        let _ = BlockPlaced {
            chunk: Entity::PLACEHOLDER,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos: BlockPos::new(0, 0, 0),
            old_state: BlockStateId(0).into(),
            new_state: BlockStateId(0).into(),
            flags: BlockUpdateFlags::all(),
        };
    }
}
