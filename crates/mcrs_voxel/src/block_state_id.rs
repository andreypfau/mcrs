#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BlockStateId(u16);

impl BlockStateId {
    pub const AIR: BlockStateId = BlockStateId(0);

    pub const fn checked(number: u16, len: usize) -> Option<Self> {
        if (number as usize) < len {
            Some(Self(number))
        } else {
            None
        }
    }

    pub const fn number(self) -> u16 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl From<BlockStateId> for u16 {
    fn from(id: BlockStateId) -> Self {
        id.0
    }
}
