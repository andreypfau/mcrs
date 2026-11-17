use std::fmt;

use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::concrete_pairs::CONCRETE_PAIRS_PATH;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcretePairsError {
    pub pack: String,
    pub identifier: ResourceLocationBuf,
    pub kind: ConcretePairsErrorKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConcretePairsErrorKind {
    UnknownBlock,
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
