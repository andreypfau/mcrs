use mcrs_voxel::BlockStateId;

/// A block state as it travels on the wire, not yet checked against the block definitions. The
/// side that holds the definitions turns it into a [`BlockStateId`] with
/// `BlockStateId::checked(number, definitions.state_count())`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockStateNumber(pub u16);

impl From<BlockStateId> for BlockStateNumber {
    fn from(id: BlockStateId) -> Self {
        Self(id.number())
    }
}
