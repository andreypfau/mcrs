use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PredicateIssue {
    UnloadedBlockTags,
    UnloadedFluidTags,
    UnloadedBiomeTags,
    UnloadedStructureTags,
    UnloadedDimensions,
    UnloadedItemTags,
    UnloadedEnchantmentTags,
    UnloadedAttributeTags,
    UnnamedEntry,
    UnreadableComponent,
    UnloadedEntityTypeTags,
    UnloadedEntityTypes,
    UnloadedDamageTypeTags,
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
            PredicateIssue::UnloadedDamageTypeTags => "damage type tags",
        };
        write!(f, "the registry set has no {missing}")
    }
}

impl core::error::Error for PredicateIssue {}
