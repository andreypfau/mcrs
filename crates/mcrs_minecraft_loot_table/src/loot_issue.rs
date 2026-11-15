use core::fmt;

use mcrs_minecraft_loot_condition::{ConditionProblem, ParamNotAllowed};
use mcrs_minecraft_resource::ResourceLocationBuf;

/// A problem that the bake found in a table. A name that the bake could not resolve makes the
/// part that names it never pass or make nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootIssue {
    pub table: ResourceLocationBuf,
    /// The part of the table with the problem, as `pools[2].entries[0].condition`. It is empty
    /// for the table as a whole.
    pub path: String,
    pub problem: LootProblem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootProblem {
    /// A block names a table that the load does not hold.
    MissingTable,
    UnknownItem(String),
    UnknownEnchantment(String),
    /// A name in a condition or a provider of the table.
    Condition(ConditionProblem),
    /// The part reads a parameter that the type of the table does not allow (LOOT-CTX-013).
    ParamNotAllowed(ParamNotAllowed),
}

impl fmt::Display for LootIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let table = &self.table;
        if self.path.is_empty() {
            write!(f, "{table}: ")?;
        } else {
            write!(f, "{table} at {}: ", self.path)?;
        }
        match &self.problem {
            LootProblem::MissingTable => f.write_str("the loot table is not loaded"),
            LootProblem::UnknownItem(name) => write!(f, "no item {name}"),
            LootProblem::UnknownEnchantment(name) => write!(f, "no enchantment {name}"),
            LootProblem::Condition(problem) => write!(f, "{problem}"),
            LootProblem::ParamNotAllowed(problem) => write!(f, "{problem}"),
        }
    }
}
