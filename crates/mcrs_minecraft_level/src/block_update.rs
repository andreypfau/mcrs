use crate::block::BlockUpdateFlags;
pub use crate::voxel_update::{BlockPlaced, BlockSetRequest, BlockUpdatePlugin};
use bevy_ecs::entity::Entity;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;

pub fn remove_block<P: Into<BlockPos>>(dimension: Entity, pos: P) -> BlockSetRequest {
    BlockSetRequest {
        dimension,
        pos: pos.into(),
        new_state: VoxelId(0),
        flags: BlockUpdateFlags::all(),
        recursion_left: 512,
    }
}
