use core::fmt;

/// A name in a condition or a provider that the bake could not resolve. The part that names it
/// never passes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConditionProblem {
    /// No item has the name or the tag.
    UnknownItem(String),
    /// No enchantment has the name or the tag.
    UnknownEnchantment(String),
    /// No block has the name or the tag.
    UnknownBlock(String),
    /// No registered predicate has the key or the tag, or the predicates name each other too
    /// deep.
    UnknownPredicate(String),
    /// A state test names a property that the block does not have.
    UnknownProperty { block: String, property: String },
}

impl fmt::Display for ConditionProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionProblem::UnknownItem(name) => write!(f, "no item {name}"),
            ConditionProblem::UnknownEnchantment(name) => write!(f, "no enchantment {name}"),
            ConditionProblem::UnknownBlock(name) => write!(f, "no block {name}"),
            ConditionProblem::UnknownPredicate(name) => write!(f, "no predicate {name}"),
            ConditionProblem::UnknownProperty { block, property } => {
                write!(f, "the block {block} has no property {property}")
            }
        }
    }
}

impl core::error::Error for ConditionProblem {}
