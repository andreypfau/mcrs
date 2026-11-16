use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot::slot_source_type_keys::SlotSourceType;
use mcrs_minecraft_loot_condition::{ConditionFile, FloatProviderFile, IntProviderFile};
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
    pub rolls: IntProviderFile,
    pub bonus_rolls: Option<FloatProviderFile>,
    #[serde(default)]
    pub entries: Vec<EntryFile>,
}

/// An entry of REG-FLD-790. A composite entry has no `weight` and no `quality`, so the bake
/// ignores them there.
#[derive(Debug, Clone, Deserialize)]
pub struct EntryFile {
    pub condition: Option<ConditionFile>,
    pub modifier: Option<ModifierFile>,
    #[serde(default = "one")]
    pub weight: i32,
    #[serde(default)]
    pub quality: i32,
    #[serde(flatten)]
    pub kind: EntryKind,
}

fn generic() -> ContextKeySet {
    ContextKeySet::Generic
}

fn one() -> i32 {
    1
}

/// The type of an entry; a type without a variant fails the decode.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum EntryKind {
    #[serde(rename = "minecraft:item", alias = "item")]
    Item { name: ResourceLocationBuf },
    #[serde(rename = "minecraft:tag", alias = "tag")]
    Tag {
        items: ItemSetFile,
        #[serde(default)]
        expand: bool,
    },
    #[serde(rename = "minecraft:loot_table", alias = "loot_table")]
    LootTable {
        value: TableSetFile,
        #[serde(default)]
        expand: bool,
    },
    #[serde(rename = "minecraft:dynamic", alias = "dynamic")]
    Dynamic { name: ResourceLocationBuf },
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty,
    #[serde(rename = "minecraft:slots", alias = "slots")]
    Slots { slot_source: SlotSourceFile },
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
}

/// The `items` of a `tag` entry: a tag, a list of items, or one item (REG-FLD-793).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ItemSetFile {
    Tag(TagName),
    Many(Vec<ResourceLocationBuf>),
    One(ResourceLocationBuf),
}

/// The `value` of a `loot_table` entry: a tag of tables, a list, or one element (REG-FLD-794).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TableSetFile {
    Tag(TagName),
    Many(Vec<TableFile>),
    One(TableFile),
}

/// A slot source (REG-FLD-801): a key, the bare `terms` of a `group`, or an object with a
/// `type`. The fields of a typed object are not decoded yet.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SlotSourceFile {
    Key(ResourceLocationBuf),
    Group(Vec<SlotSourceFile>),
    Typed {
        #[serde(rename = "type")]
        kind: SlotSourceType,
    },
}

/// The key of a table, or an inline table.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TableFile {
    Key(ResourceLocationBuf),
    Inline(Box<LootTableFile>),
}

/// The name of a tag, written with the prefix `#`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct TagName(pub ResourceLocationBuf);

impl TryFrom<String> for TagName {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let Some(name) = text.strip_prefix('#') else {
            return Err(format!("{text} is not a tag"));
        };
        ResourceLocationBuf::read(name)
            .map(TagName)
            .map_err(|error| error.to_string())
    }
}
