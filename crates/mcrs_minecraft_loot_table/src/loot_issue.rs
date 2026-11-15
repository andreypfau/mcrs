use core::fmt;

use mcrs_minecraft_loot_condition::{ConditionProblem, ParamNotAllowed};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::table_links::MAX_NESTING;

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
    /// A `set_item` names `minecraft:air` (REG-FLD-744).
    SetItemAir,
    /// A `loot_table` entry names a table that the load does not hold.
    UnknownTable(String),
    /// A tag of items or of tables that the load does not hold.
    UnknownTag(String),
    /// A name in a condition or a provider of the table.
    Condition(ConditionProblem),
    /// The part reads a parameter that the type of the table does not allow, or a nested table
    /// reads one that the type of the table that contains it does not allow (LOOT-CTX-013).
    ParamNotAllowed(ParamNotAllowed),
    /// The `loot_table` entry refers back to a table that contains it, through these tables
    /// from the first to the last; the last refers back to the first (LOOT-CTX-014).
    Cycle(Vec<ResourceLocationBuf>),
    /// The `loot_table` entry names a table whose chain of nested tables is already as long as
    /// the bake allows.
    TooDeep,
    /// A composite entry without children (LOOT-CTX-016).
    NoChildren,
    /// A child of an `alternatives` entry, other than the last child, without a condition
    /// (LOOT-CTX-016).
    AlternativeWithoutCondition,
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
            LootProblem::SetItemAir => f.write_str("set_item names minecraft:air"),
            LootProblem::UnknownTable(name) => write!(f, "no loot table {name}"),
            LootProblem::UnknownTag(name) => write!(f, "no tag #{name}"),
            LootProblem::Condition(problem) => write!(f, "{problem}"),
            LootProblem::ParamNotAllowed(problem) => write!(f, "{problem}"),
            LootProblem::Cycle(tables) => {
                f.write_str("the loot table refers back to itself through")?;
                for table in tables {
                    write!(f, " {table}")?;
                }
                Ok(())
            }
            LootProblem::TooDeep => {
                write!(f, "more than {MAX_NESTING} nested loot tables in one chain")
            }
            LootProblem::NoChildren => f.write_str("the composite entry has no children"),
            LootProblem::AlternativeWithoutCondition => {
                f.write_str("a child of an alternatives entry other than the last has no condition")
            }
        }
    }
}
