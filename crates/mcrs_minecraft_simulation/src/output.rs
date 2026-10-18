use crate::BlockUpdateFlags;
use mcrs_minecraft_core::{BlockPos, BlockStateId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockWrite {
    pub pos: BlockPos,
    pub old: BlockStateId,
    pub new: BlockStateId,
    pub flags: BlockUpdateFlags,
}
