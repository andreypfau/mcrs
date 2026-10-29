use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::Named;
use serde::{Deserialize, Serialize};

use crate::Text;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "LootTable", default, skip_serializing_if = "Option::is_none")]
    pub loot_table: Option<String>,
    #[serde(rename = "LootTableSeed", default, skip_serializing_if = "is_default")]
    pub loot_table_seed: i64,
    #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ItemStackWithSlot<Named>>,
    #[serde(
        rename = "CustomName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_name: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

impl ContainerData {
    pub fn looted(pos: BlockPos, loot_table: String, loot_table_seed: i64) -> Self {
        ContainerData {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            loot_table: Some(loot_table),
            loot_table_seed,
            items: Vec::new(),
            custom_name: None,
            components: None,
        }
    }
}
