use std::collections::BTreeMap;

use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::Named;
use serde::{Deserialize, Serialize};

use crate::Text;

fn one_f32() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FurnaceData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default)]
    pub cooking_time_spent: i32,
    #[serde(default)]
    pub cooking_total_time: i32,
    #[serde(default)]
    pub lit_time_remaining: i32,
    #[serde(default)]
    pub lit_total_time: i32,
    #[serde(default = "one_f32")]
    pub speed_multiplier: f32,
    #[serde(rename = "Items", default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ItemStackWithSlot<Named>>,
    #[serde(
        rename = "RecipesUsed",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub recipes_used: BTreeMap<String, i32>,
    #[serde(
        rename = "CustomName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_name: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}
