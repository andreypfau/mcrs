use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::condition_file::ConditionFile;
use crate::modifier_file::ModifierFile;
use crate::number_file::NumberFile;

/// A loot table as a data pack writes it.
#[derive(Debug, Clone, Deserialize)]
pub struct LootTableFile {
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
