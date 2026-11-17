use crate::concrete_pairs::ConcretePairsFile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackConcretePairs {
    pub pack: String,
    pub file: ConcretePairsFile,
}
