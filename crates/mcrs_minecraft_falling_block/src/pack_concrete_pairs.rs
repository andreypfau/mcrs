use crate::concrete_pairs::ConcretePairsFile;

/// The concrete pairs file of one data pack, by the name of the pack (FBK-TRG-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackConcretePairs {
    /// The name of the pack, which an error names.
    pub pack: String,
    /// The concrete pairs file of the pack.
    pub file: ConcretePairsFile,
}
