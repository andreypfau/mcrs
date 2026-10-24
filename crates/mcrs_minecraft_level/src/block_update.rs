pub use crate::voxel_update::{BlockPlaced, BlockSetRequest, BlockUpdatePlugin};
use bevy_ecs::entity::Entity;
use mcrs_minecraft_core::{BlockPos, BlockStateId};
use mcrs_minecraft_simulation::BlockUpdateFlags;

pub fn remove_block<P: Into<BlockPos>>(dimension: Entity, pos: P) -> BlockSetRequest {
    BlockSetRequest {
        dimension,
        pos: pos.into(),
        new_state: BlockStateId::AIR,
        flags: BlockUpdateFlags::all(),
        recursion_left: 512,
    }
}
