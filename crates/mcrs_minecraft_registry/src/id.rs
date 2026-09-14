use mcrs_minecraft_chunk::VoxelId;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct BlockStateId(pub u16);

impl From<u16> for BlockStateId {
    #[inline]
    fn from(id: u16) -> Self {
        BlockStateId(id)
    }
}

impl From<BlockStateId> for u16 {
    #[inline]
    fn from(id: BlockStateId) -> Self {
        id.0
    }
}

impl From<BlockStateId> for VoxelId {
    #[inline]
    fn from(id: BlockStateId) -> Self {
        VoxelId(id.0)
    }
}

impl From<VoxelId> for BlockStateId {
    #[inline]
    fn from(id: VoxelId) -> Self {
        BlockStateId(id.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct ItemId(pub u16);

impl From<u16> for ItemId {
    #[inline]
    fn from(id: u16) -> Self {
        ItemId(id)
    }
}

impl From<ItemId> for u16 {
    #[inline]
    fn from(id: ItemId) -> Self {
        id.0
    }
}
