use alloc::borrow::Cow;

use crate::BlockId;
use crate::BlockTagId;

/// The blocks that a tool rule applies to. A tag is never equal to a list, whatever its blocks.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockSet {
    Tag(BlockTagId),
    Blocks(Cow<'static, [BlockId]>),
}
