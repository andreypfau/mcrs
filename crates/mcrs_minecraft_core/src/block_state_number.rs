use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockStateNumber(pub u16);

impl From<BlockStateId> for BlockStateNumber {
    fn from(id: BlockStateId) -> Self {
        Self(id.number())
    }
}
