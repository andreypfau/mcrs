use mcrs_minecraft_falling_block::ConcretePairsFile;

/// The concrete pairs file of one data pack, by the name of the pack (FBK-TRG-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackConcretePairs {
    pub pack: String,
    pub file: ConcretePairsFile,
}
