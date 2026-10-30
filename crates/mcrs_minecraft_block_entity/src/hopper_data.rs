use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::Named;

use crate::{ContainerData, Text};

fn minus_one() -> i32 {
    -1
}

/// A hopper saves like any container, plus its transfer cooldown.
#[derive(Clone, Debug, PartialEq)]
pub struct HopperData {
    pub container: ContainerData,
    pub transfer_cooldown: i32,
}

#[derive(Deserialize)]
struct HopperFields {
    x: i32,
    y: i32,
    z: i32,
    #[serde(rename = "LootTable", default)]
    loot_table: Option<String>,
    #[serde(rename = "LootTableSeed", default)]
    loot_table_seed: i64,
    #[serde(rename = "Items", default)]
    items: Vec<ItemStackWithSlot<Named>>,
    #[serde(rename = "CustomName", default)]
    custom_name: Option<Text>,
    #[serde(default)]
    components: Option<NbtCompound>,
    #[serde(rename = "TransferCooldown", default = "minus_one")]
    transfer_cooldown: i32,
}

impl<'de> Deserialize<'de> for HopperData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = HopperFields::deserialize(deserializer)?;
        Ok(HopperData {
            container: ContainerData {
                x: fields.x,
                y: fields.y,
                z: fields.z,
                loot_table: fields.loot_table,
                loot_table_seed: fields.loot_table_seed,
                items: fields.items,
                custom_name: fields.custom_name,
                components: fields.components,
            },
            transfer_cooldown: fields.transfer_cooldown,
        })
    }
}

impl Serialize for HopperData {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("HopperData", 9)?;
        self.container.serialize_fields(&mut out)?;
        out.serialize_field("TransferCooldown", &self.transfer_cooldown)?;
        out.end()
    }
}
