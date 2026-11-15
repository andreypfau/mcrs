use core::fmt;

/// A name in a condition or a provider that the bake could not resolve, or a provider that the
/// load refuses. A condition with the problem never passes, and a provider with it gives an
/// evaluation error.
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
    /// No named provider has the key or the tag.
    UnknownProvider(String),
    /// A reference chain enters a named provider that it already entered (LOOT-CTX-014). The
    /// text is the path of the chain.
    ProviderCycle(String),
    /// A reference chain of providers gets a length of more than 64 (LOOT-CTX-017). The text is
    /// the path of the chain.
    ProviderChainTooLong(String),
    /// An aggregate provider has no `inputs` (LOOT-CTX-016). The text is the path of the
    /// provider in its root provider, empty for the root.
    EmptyInputs(String),
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
            ConditionProblem::UnknownProvider(name) => write!(f, "no provider {name}"),
            ConditionProblem::ProviderCycle(path) => {
                write!(f, "the provider refers back to itself: {path}")
            }
            ConditionProblem::ProviderChainTooLong(path) => {
                write!(f, "the reference chain is longer than 64: {path}")
            }
            ConditionProblem::EmptyInputs(path) if path.is_empty() => {
                f.write_str("the provider has no inputs")
            }
            ConditionProblem::EmptyInputs(path) => {
                write!(f, "the provider at {path} has no inputs")
            }
        }
    }
}

impl core::error::Error for ConditionProblem {}
