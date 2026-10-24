use mcrs_minecraft_core::BlockPos;

use crate::access::{Denied, Unavailable, Volume};

/// A volume that keeps block entities, at most one per position.
pub trait BlockEntityVolume: Volume {
    type BlockEntity;

    /// `Ok(None)` where the position is readable and holds none.
    fn block_entity(&self, at: BlockPos) -> Result<Option<&Self::BlockEntity>, Unavailable>;
}

/// A volume whose block entities can be written. Like a block write, storage only.
pub trait BlockEntityVolumeMut: BlockEntityVolume {
    /// Answers the block entity it replaced.
    fn set_block_entity(
        &mut self,
        at: BlockPos,
        entity: Self::BlockEntity,
    ) -> Result<Option<Self::BlockEntity>, Denied>;

    fn remove_block_entity(&mut self, at: BlockPos) -> Result<Option<Self::BlockEntity>, Denied>;

    /// `Ok(None)` where the position is writable and holds none.
    fn block_entity_mut(&mut self, at: BlockPos) -> Result<Option<&mut Self::BlockEntity>, Denied>;
}
