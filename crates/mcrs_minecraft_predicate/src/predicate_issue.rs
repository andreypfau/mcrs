use core::fmt;

/// A fault that the bake of a predicate finds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PredicateIssue {
    /// The registry set has no tags of blocks.
    UnloadedBlockTags,
    /// The registry set has no tags of fluids.
    UnloadedFluidTags,
    /// The registry set has no tags of biomes.
    UnloadedBiomeTags,
    /// The registry set has no tags of structures.
    UnloadedStructureTags,
    /// The registry set has no dimension registry.
    UnloadedDimensions,
}

impl fmt::Display for PredicateIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let missing = match self {
            PredicateIssue::UnloadedBlockTags => "block tags",
            PredicateIssue::UnloadedFluidTags => "fluid tags",
            PredicateIssue::UnloadedBiomeTags => "biome tags",
            PredicateIssue::UnloadedStructureTags => "structure tags",
            PredicateIssue::UnloadedDimensions => "dimension registry",
        };
        write!(f, "the registry set has no {missing}")
    }
}

impl core::error::Error for PredicateIssue {}
