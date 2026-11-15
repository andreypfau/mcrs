use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{ConditionFile, NumberFile};
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::modifier_file::ModifierFile;

/// A loot table as a data pack writes it.
#[derive(Debug, Clone, Deserialize)]
pub struct LootTableFile {
    /// The context key set of the table (LOOT-CTX-013).
    #[serde(rename = "type", default = "generic")]
    pub key_set: ContextKeySet,
    #[serde(default)]
    pub pools: Vec<PoolFile>,
    pub modifier: Option<ModifierFile>,
    pub random_sequence: Option<ResourceLocationBuf>,
}

impl mcrs_minecraft_resource::RegistryValue for LootTableFile {
    type Registry = mcrs_minecraft_loot::loot::LootTable;
}

mcrs_minecraft_registries::resolve_as_is!(LootTableFile);

#[derive(Debug, Clone, Deserialize)]
pub struct PoolFile {
    pub condition: Option<ConditionFile>,
    pub modifier: Option<ModifierFile>,
    pub rolls: NumberFile,
    pub bonus_rolls: Option<NumberFile>,
    #[serde(default)]
    pub entries: Vec<EntryFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryFile {
    pub condition: Option<ConditionFile>,
    pub modifier: Option<ModifierFile>,
    #[serde(default = "one")]
    pub weight: i32,
    #[serde(flatten)]
    pub kind: EntryKind,
}

fn generic() -> ContextKeySet {
    ContextKeySet::Generic
}

fn one() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum EntryKind {
    #[serde(rename = "minecraft:item", alias = "item")]
    Item { name: ResourceLocationBuf },
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty,
    #[serde(rename = "minecraft:alternatives", alias = "alternatives")]
    Alternatives {
        #[serde(default)]
        children: Vec<EntryFile>,
    },
    #[serde(rename = "minecraft:sequence", alias = "sequence")]
    Sequence {
        #[serde(default)]
        children: Vec<EntryFile>,
    },
    #[serde(rename = "minecraft:group", alias = "group")]
    Group {
        #[serde(default)]
        children: Vec<EntryFile>,
    },
    #[serde(other)]
    Other,
}
