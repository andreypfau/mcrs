use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_item::keys::ContextKeySet;
use mcrs_minecraft_item::loot::LootTable;
use mcrs_minecraft_registry::Holder;
use serde::{Deserialize, Deserializer, Serialize};

use crate::condition::LootCondition;
use crate::entry::LootPoolEntry;
use crate::function::LootItemFunction;
use crate::number::{FloatExpression, IntExpression, is_zero_holder, zero_holder};

/// A loot table as its file states it: the context it is rolled in, its random
/// sequence, its pools and the modifier applied to everything it drops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LootTableFile {
    #[serde(
        rename = "type",
        default = "generic",
        deserialize_with = "lenient_context",
        skip_serializing_if = "is_generic"
    )]
    pub context: ContextKeySet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_sequence: Option<ResourceLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pools: Vec<LootPool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Holder<LootItemFunction>>,
}

impl RegistryValue for LootTableFile {
    type Registry = LootTable;
}

/// The pools and the table-wide modifier: a column beside the loot table
/// registry, whose value is what the item crate names.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LootTableBody {
    pub pools: Vec<LootPool>,
    pub modifier: Option<Holder<LootItemFunction>>,
}

impl LootTableFile {
    pub fn split(file: &Self) -> (LootTable, LootTableBody) {
        (
            LootTable {
                context: file.context,
                random_sequence: file.random_sequence.clone(),
            },
            LootTableBody {
                pools: file.pools.clone(),
                modifier: file.modifier.clone(),
            },
        )
    }

    pub fn join((table, body): (&LootTable, &LootTableBody)) -> Self {
        LootTableFile {
            context: table.context,
            random_sequence: table.random_sequence.clone(),
            pools: body.pools.clone(),
            modifier: body.modifier.clone(),
        }
    }
}

fn generic() -> ContextKeySet {
    ContextKeySet::Generic
}

fn is_generic(context: &ContextKeySet) -> bool {
    *context == ContextKeySet::Generic
}

/// The game reads an unknown context as the generic one rather than refusing it.
fn lenient_context<'de, D: Deserializer<'de>>(d: D) -> Result<ContextKeySet, D::Error> {
    let text = String::deserialize(d)?;
    Ok(ResourceLocation::read(&text)
        .ok()
        .and_then(|id| ContextKeySet::find(id.as_str()))
        .unwrap_or(ContextKeySet::Generic))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LootPool {
    pub entries: Vec<LootPoolEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Holder<LootCondition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Holder<LootItemFunction>>,
    pub rolls: Holder<IntExpression>,
    #[serde(default = "zero_holder", skip_serializing_if = "is_zero_holder")]
    pub bonus_rolls: Holder<FloatExpression>,
}
