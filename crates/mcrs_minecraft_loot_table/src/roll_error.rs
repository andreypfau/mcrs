use core::fmt;

use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::ContextParam;
use mcrs_minecraft_resource::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RollError {
    UnknownTable(Id<LootTable>),
    MissingParam(ContextParam),
    UnexpectedParam(ContextParam),
}

impl fmt::Display for RollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RollError::UnknownTable(table) => write!(f, "no loot table {}", table.number()),
            RollError::MissingParam(param) => {
                write!(f, "the context lacks the required parameter {param}")
            }
            RollError::UnexpectedParam(param) => {
                write!(
                    f,
                    "the context holds the parameter {param}, which its key set does not allow"
                )
            }
        }
    }
}

impl core::error::Error for RollError {}
