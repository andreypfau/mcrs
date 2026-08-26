use crate::world::loot::condition::LootConditionProto;
use mcrs_core::ResourceLocation;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum LootEntryProto {
    #[serde(rename = "minecraft:item")]
    Item {
        name: ResourceLocation,
        #[serde(default)]
        conditions: Vec<LootConditionProto>,
        #[serde(default)]
        functions: Vec<serde_json::Value>,
    },
    #[serde(rename = "minecraft:alternatives")]
    Alternatives {
        children: Vec<LootEntryProto>,
        #[serde(default)]
        conditions: Vec<LootConditionProto>,
    },
    #[serde(rename = "minecraft:empty")]
    Empty {
        #[serde(default)]
        conditions: Vec<LootConditionProto>,
    },
    #[serde(other)]
    Unknown,
}
