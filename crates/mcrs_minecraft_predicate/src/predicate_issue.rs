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
    /// The registry set has no tags of items.
    UnloadedItemTags,
    /// The registry set has no tags of enchantments.
    UnloadedEnchantmentTags,
    /// The registry set has no tags of attributes.
    UnloadedAttributeTags,
    /// An entry of `components` holds an id that the registry set does not name.
    UnnamedEntry,
    /// The stack codec rejects the saved form of an entry of `components`.
    UnreadableComponent,
    /// The registry set has no tags of entity types.
    UnloadedEntityTypeTags,
    /// The registry set has no entity type registry.
    UnloadedEntityTypes,
    /// One path of nested entity predicates holds more entity predicates than the bake accepts.
    NestingTooDeep,
}

impl fmt::Display for PredicateIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let missing = match self {
            PredicateIssue::NestingTooDeep => {
                return f.write_str("the entity predicates nest too deep");
            }
            PredicateIssue::UnloadedBlockTags => "block tags",
            PredicateIssue::UnloadedFluidTags => "fluid tags",
            PredicateIssue::UnloadedBiomeTags => "biome tags",
            PredicateIssue::UnloadedStructureTags => "structure tags",
            PredicateIssue::UnloadedDimensions => "dimension registry",
            PredicateIssue::UnloadedItemTags => "item tags",
            PredicateIssue::UnloadedEnchantmentTags => "enchantment tags",
            PredicateIssue::UnloadedAttributeTags => "attribute tags",
            PredicateIssue::UnnamedEntry => {
                return f.write_str("a component of the predicate holds an unnamed id");
            }
            PredicateIssue::UnreadableComponent => {
                return f.write_str("the stack codec rejects a component of the predicate");
            }
            PredicateIssue::UnloadedEntityTypeTags => "entity type tags",
            PredicateIssue::UnloadedEntityTypes => "entity type registry",
        };
        write!(f, "the registry set has no {missing}")
    }
}

impl core::error::Error for PredicateIssue {}
