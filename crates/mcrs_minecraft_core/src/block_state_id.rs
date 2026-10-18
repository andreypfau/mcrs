/// The number of a block state. A block-state id is made only by checking a number against the
/// length of the table that issues it.
///
/// ```
/// use mcrs_minecraft_core::BlockStateId;
///
/// assert!(BlockStateId::checked(0, 0).is_none());
/// assert!(BlockStateId::checked(0, 1).is_some());
/// assert!(BlockStateId::checked(1, 1).is_none());
/// let last = BlockStateId::checked(65535, 65536).unwrap();
/// assert_eq!((last.number(), last.index()), (65535, 65535));
/// assert_eq!(BlockStateId::AIR.number(), 0);
/// ```
///
/// ```compile_fail,E0423
/// use mcrs_minecraft_core::BlockStateId;
///
/// let _ = BlockStateId(5);
/// ```
///
/// ```compile_fail,E0308
/// use mcrs_minecraft_core::BlockStateId;
///
/// let _ = BlockStateId::from(5u16);
/// ```
///
/// ```compile_fail,E0277
/// use mcrs_minecraft_core::BlockStateId;
///
/// let _ = <BlockStateId as Default>::default();
/// ```
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
