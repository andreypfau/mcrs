/// A block state's numeric id: an index into the block state table of the data pack that
/// was loaded, so it is not stable across versions or packs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug)]
pub struct BlockStateId(pub u16);
