use std::fmt;

use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::concrete_pairs::CONCRETE_PAIRS_PATH;

/// A pair of the concrete pairs that makes the load of the data packs fail (FBK-TRG-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcretePairsError {
    pub pack: String,
    pub identifier: ResourceLocationBuf,
    pub kind: ConcretePairsErrorKind,
}

/// The reason of a [`ConcretePairsError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConcretePairsErrorKind {
    /// A key or a value is not a block of the block registry.
    UnknownBlock,
    /// A key is equal to its value.
    MapsToItself,
}

impl fmt::Display for ConcretePairsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.kind {
            ConcretePairsErrorKind::UnknownBlock => "is not a block",
            ConcretePairsErrorKind::MapsToItself => "maps to itself",
        };
        write!(
            formatter,
            "data pack {}, {CONCRETE_PAIRS_PATH}: {} {reason}",
            self.pack, self.identifier
        )
    }
}

impl core::error::Error for ConcretePairsError {}
