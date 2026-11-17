use core::fmt;

use mcrs_minecraft_predicate::PredicateIssue;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConditionProblem {
    UnknownEnchantment(String),
    UnknownClock(String),
    UnknownPredicate(String),
    UnknownProvider(String),
    Unresolved { path: String, message: String },
    Predicate(PredicateIssue),
    Cycle(String),
    ChainTooLong(String),
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
