use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registry::{Holder, HolderList, HolderSet};
use serde::{Deserialize, Serialize};

use crate::condition::LootCondition;
use crate::function::LootItemFunction;
use crate::slot::SlotSource;
use crate::table::LootTableFile;

/// One way a pool can roll, dispatched on `loot_pool_entry_type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LootPoolEntry {
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty(Uniform),
    #[serde(rename = "minecraft:item", alias = "item")]
    Item(ItemEntry),
    #[serde(rename = "minecraft:loot_table", alias = "loot_table")]
    LootTable(NestedLootTable),
    #[serde(rename = "minecraft:dynamic", alias = "dynamic")]
    Dynamic(DynamicEntry),
    #[serde(rename = "minecraft:tag", alias = "tag")]
    Tag(TagEntry),
    #[serde(rename = "minecraft:slots", alias = "slots")]
    Slots(SlotsEntry),
    #[serde(rename = "minecraft:alternatives", alias = "alternatives")]
    Alternatives(Composite),
    #[serde(rename = "minecraft:sequence", alias = "sequence")]
    Sequence(Composite),
    #[serde(rename = "minecraft:group", alias = "group")]
    Group(Composite),
}

fn default_weight() -> i32 {
    1
}

fn is_default_weight(weight: &i32) -> bool {
    *weight == 1
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

/// An entry with nothing to roll beyond its weight, quality, condition and
/// modifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uniform {
    #[serde(default = "default_weight", skip_serializing_if = "is_default_weight")]
    pub weight: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub quality: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Holder<LootCondition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Holder<LootItemFunction>>,
}

macro_rules! entry {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fmeta:meta])* pub $field:ident : $ty:ty,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
            #[serde(default = "default_weight", skip_serializing_if = "is_default_weight")]
            pub weight: i32,
            #[serde(default, skip_serializing_if = "is_zero")]
            pub quality: i32,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub condition: Option<Holder<LootCondition>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub modifier: Option<Holder<LootItemFunction>>,
        }
    };
}

entry! {
    pub struct ItemEntry {
        pub name: Item,
    }
}

entry! {
    pub struct NestedLootTable {
        pub value: HolderList<LootTableFile>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub expand: bool,
    }
}

entry! {
    pub struct DynamicEntry {
        pub name: ResourceLocation,
    }
}

entry! {
    pub struct TagEntry {
        pub items: HolderSet<Item>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub expand: bool,
    }
}

entry! {
    pub struct SlotsEntry {
        pub slot_source: Holder<SlotSource>,
    }
}

/// Entries rolled as alternatives, in sequence, or as a group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composite {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<LootPoolEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Holder<LootCondition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Holder<LootItemFunction>>,
}
