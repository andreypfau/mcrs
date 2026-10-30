use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::Named;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::Text;

/// A container saves its loot table instead of its items; any other container saves
/// `Items`, even when empty.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ContainerData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "LootTable", default)]
    pub loot_table: Option<String>,
    #[serde(rename = "LootTableSeed", default)]
    pub loot_table_seed: i64,
    #[serde(rename = "Items", default)]
    pub items: Vec<ItemStackWithSlot<Named>>,
    #[serde(rename = "CustomName", default)]
    pub custom_name: Option<Text>,
    #[serde(default)]
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

impl ContainerData {
    pub(crate) fn serialize_fields<S: SerializeStruct>(&self, out: &mut S) -> Result<(), S::Error> {
        out.serialize_field("x", &self.x)?;
        out.serialize_field("y", &self.y)?;
        out.serialize_field("z", &self.z)?;
        if let Some(loot_table) = &self.loot_table {
            out.serialize_field("LootTable", loot_table)?;
            if self.loot_table_seed != 0 {
                out.serialize_field("LootTableSeed", &self.loot_table_seed)?;
            }
        } else {
            out.serialize_field("Items", &self.items)?;
        }
        if let Some(custom_name) = &self.custom_name {
            out.serialize_field("CustomName", custom_name)?;
        }
        if let Some(components) = &self.components {
            out.serialize_field("components", components)?;
        }
        Ok(())
    }
}

impl Serialize for ContainerData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("ContainerData", 8)?;
        self.serialize_fields(&mut out)?;
        out.end()
    }
}
