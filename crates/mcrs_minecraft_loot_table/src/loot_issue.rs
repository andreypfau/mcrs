use core::fmt;

use mcrs_minecraft_resource::ResourceLocationBuf;

/// A name in a block table that the bake could not resolve. The part that names it never
/// passes or makes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootIssue {
    pub table: ResourceLocationBuf,
    pub problem: LootProblem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootProblem {
    MissingTable,
    UnknownItem(String),
    UnknownEnchantment(String),
    UnknownBlock(String),
    UnknownPredicate(String),
    UnknownProperty { block: String, property: String },
}

impl fmt::Display for LootIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let table = &self.table;
        match &self.problem {
            LootProblem::MissingTable => write!(f, "the loot table {table} is not loaded"),
            LootProblem::UnknownItem(name) => write!(f, "{table}: no item {name}"),
            LootProblem::UnknownEnchantment(name) => write!(f, "{table}: no enchantment {name}"),
            LootProblem::UnknownBlock(name) => write!(f, "{table}: no block {name}"),
            LootProblem::UnknownPredicate(name) => write!(f, "{table}: no predicate {name}"),
            LootProblem::UnknownProperty { block, property } => {
                write!(f, "{table}: the block {block} has no property {property}")
            }
        }
    }
}
