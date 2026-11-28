use core::fmt;

use mcrs_minecraft_loot_condition::{ConditionProblem, ParamNotAllowed};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::issue_owner::IssueOwner;
use crate::table_links::MAX_NESTING;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootIssue {
    pub name: ResourceLocationBuf,
    pub owner: IssueOwner,
    pub path: String,
    pub problem: LootProblem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootProblem {
    MissingTable,
    UnknownItem(String),
    UnknownEnchantment(String),
    SetItemAir,
    UnknownTable(String),
    UnknownTag(String),
    UnknownModifier(String),
    UnknownAttribute(String),
    Condition(ConditionProblem),
    ParamNotAllowed(ParamNotAllowed),
    Cycle(Vec<ResourceLocationBuf>),
    TooDeep,
    ChainTooLong,
    NoChildren,
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
