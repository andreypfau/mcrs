use core::fmt;

use mcrs_minecraft_loot_condition::{ConditionProblem, ParamNotAllowed};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::issue_owner::IssueOwner;
use crate::table_links::MAX_NESTING;

/// A problem that the bake found in a table. A name that the bake could not resolve makes the
/// part that names it never pass or make nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootIssue {
    /// The name of the table, the item modifier or the named entry with the problem.
    pub name: ResourceLocationBuf,
    /// The registry of `name`.
    pub owner: IssueOwner,
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
    /// A tag of items, of tables or of item modifiers that the load does not hold.
    UnknownTag(String),
    /// A key of an item modifier that the load does not hold.
    UnknownModifier(String),
    /// A `set_attributes` element names an attribute that the load does not hold, or one whose
    /// index does not fit an attribute id.
    UnknownAttribute(String),
    /// A name in a condition or a provider of the table, or a problem of a named entry.
    Condition(ConditionProblem),
    /// The part reads a parameter that the type of the table does not allow, or a nested table
    /// reads one that the type of the table that contains it does not allow (LOOT-CTX-013).
    ParamNotAllowed(ParamNotAllowed),
    /// The `loot_table` entry or the key of an item modifier refers back to a table or an item
    /// modifier that contains it, through these from the first to the last; the last refers back
    /// to the first (LOOT-CTX-014).
    Cycle(Vec<ResourceLocationBuf>),
    /// The `loot_table` entry or the key of an item modifier makes a reference chain from some
    /// start longer than [`MAX_NESTING`] (LOOT-CTX-017).
    TooDeep,
    /// The part makes a reference chain from some start longer than
    /// [`MAX_NESTING`] (LOOT-CTX-017).
    ChainTooLong,
    /// A composite entry without children (LOOT-CTX-016).
    NoChildren,
    /// A child of an `alternatives` entry, other than the last child, without a condition
    /// (LOOT-CTX-016).
    AlternativeWithoutCondition,
}

impl fmt::Display for LootIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = &self.name;
        match self.owner {
            IssueOwner::Named(kind) => write!(f, "{kind} ")?,
            IssueOwner::ItemModifier => f.write_str("item modifier ")?,
            IssueOwner::Table => {}
        }
        if self.path.is_empty() {
            write!(f, "{name}: ")?;
        } else {
            write!(f, "{name} at {}: ", self.path)?;
        }
        match &self.problem {
            LootProblem::MissingTable => f.write_str("the loot table is not loaded"),
            LootProblem::UnknownItem(name) => write!(f, "no item {name}"),
            LootProblem::UnknownEnchantment(name) => write!(f, "no enchantment {name}"),
            LootProblem::SetItemAir => f.write_str("set_item names minecraft:air"),
            LootProblem::UnknownTable(name) => write!(f, "no loot table {name}"),
            LootProblem::UnknownTag(name) => write!(f, "no tag #{name}"),
            LootProblem::UnknownModifier(name) => write!(f, "no item modifier {name}"),
            LootProblem::UnknownAttribute(name) => write!(f, "no attribute {name}"),
            LootProblem::Condition(problem) => write!(f, "{problem}"),
            LootProblem::ParamNotAllowed(problem) => write!(f, "{problem}"),
            LootProblem::Cycle(tables) => {
                f.write_str("the part refers back to itself through")?;
                for table in tables {
                    write!(f, " {table}")?;
                }
                Ok(())
            }
            LootProblem::TooDeep => {
                write!(f, "more than {MAX_NESTING} nested parts in one chain")
            }
            LootProblem::ChainTooLong => {
                write!(
                    f,
                    "the part makes a reference chain longer than {MAX_NESTING}"
                )
            }
            LootProblem::NoChildren => f.write_str("the composite entry has no children"),
            LootProblem::AlternativeWithoutCondition => {
                f.write_str("a child of an alternatives entry other than the last has no condition")
            }
        }
    }
}
