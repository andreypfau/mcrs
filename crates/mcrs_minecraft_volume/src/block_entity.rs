use mcrs_voxel::BlockPos;

use crate::access::{Denied, Unavailable, Volume};

pub trait BlockEntityVolume: Volume {
    type BlockEntity;

    fn block_entity(&self, at: BlockPos) -> Result<Option<&Self::BlockEntity>, Unavailable>;
}

pub trait BlockEntityVolumeMut: BlockEntityVolume {
    fn set_block_entity(
        &mut self,
        at: BlockPos,
        entity: Self::BlockEntity,
    ) -> Result<Option<Self::BlockEntity>, Denied>;

    fn block_entity_mut(&mut self, at: BlockPos) -> Result<Option<&mut Self::BlockEntity>, Denied>;
}
