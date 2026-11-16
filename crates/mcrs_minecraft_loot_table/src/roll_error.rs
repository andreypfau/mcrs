use core::fmt;

use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::ContextParam;
use mcrs_minecraft_resource::Id;

/// Why a roll evaluated nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RollError {
    /// The tables hold no table with this id.
    UnknownTable(Id<LootTable>),
    /// The context lacks a parameter that its key set requires (LOOT-CTX-012).
    MissingParam(ContextParam),
    /// The context holds a parameter that its key set does not allow (LOOT-CTX-012).
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
