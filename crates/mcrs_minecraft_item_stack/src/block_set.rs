use alloc::borrow::Cow;

use crate::BlockId;
use crate::BlockTagId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockSet {
    Tag(BlockTagId),
    Blocks(Cow<'static, [BlockId]>),
}
