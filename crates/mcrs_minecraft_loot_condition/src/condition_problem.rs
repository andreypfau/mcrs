use core::fmt;

use mcrs_minecraft_predicate::PredicateIssue;

/// A name in a condition or a provider that the bake could not resolve, or a part that the load
/// refuses. A condition with the problem never passes, and a provider with it gives an evaluation
/// error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConditionProblem {
    /// No enchantment has the name.
    UnknownEnchantment(String),
    /// No world clock has the name.
    UnknownClock(String),
    /// No registered predicate has the key or the tag.
    UnknownPredicate(String),
    /// No named provider has the key or the tag.
    UnknownProvider(String),
    /// A name in a shared predicate does not resolve. `path` is the place of the name in the
    /// condition.
    Unresolved { path: String, message: String },
    /// The bake of a shared predicate of the condition fails.
    Predicate(PredicateIssue),
    /// A reference chain enters a named entry that it already entered (LOOT-CTX-014). The text
    /// is the path of the chain.
    Cycle(String),
    /// A reference chain gets a length of more than 64 (LOOT-CTX-017). The text is the path of
    /// the chain.
    ChainTooLong(String),
    /// An aggregate provider has no `inputs` (LOOT-CTX-016). The text is the path of the
    /// provider in its root provider, empty for the root.
    EmptyInputs(String),
}

impl fmt::Display for ConditionProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionProblem::UnknownEnchantment(name) => write!(f, "no enchantment {name}"),
            ConditionProblem::UnknownClock(name) => write!(f, "no world clock {name}"),
            ConditionProblem::UnknownPredicate(name) => write!(f, "no predicate {name}"),
            ConditionProblem::UnknownProvider(name) => write!(f, "no provider {name}"),
            ConditionProblem::Unresolved { path, message } => write!(f, "{path}: {message}"),
            ConditionProblem::Predicate(issue) => write!(f, "{issue}"),
            ConditionProblem::Cycle(path) => write!(f, "the entry refers back to itself: {path}"),
            ConditionProblem::ChainTooLong(path) => {
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
