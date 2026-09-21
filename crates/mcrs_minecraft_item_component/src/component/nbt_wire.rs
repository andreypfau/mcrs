use std::collections::BTreeMap;

use mcrs_minecraft_core::codec::{is_default, long_value};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{COMPOUND_ID, DOUBLE_ID, FLOAT_ID, LIST_ID, LONG_ID, STRING_ID};
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::{
    BlockReg, LootTableReg, MapDecorationTypeReg, RecipeReg, compound_or_snbt,
};
use crate::harness::Sample;

/// The compound as is; an SNBT string reads as one too.
#[derive(Clone, Debug, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct CustomData(pub NbtCompound);

impl<'de> Deserialize<'de> for CustomData {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        compound_or_snbt(d).map(CustomData)
    }
}

impl Sample for CustomData {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", mcrs_minecraft_nbt::COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.extend([
                ("flag", mcrs_minecraft_nbt::BYTE_ID),
                ("count", mcrs_minecraft_nbt::SHORT_ID),
                ("big", mcrs_minecraft_nbt::INT_ID),
                ("seed", mcrs_minecraft_nbt::LONG_ID),
                ("ratio", mcrs_minecraft_nbt::FLOAT_ID),
                ("precise", mcrs_minecraft_nbt::DOUBLE_ID),
                ("nested", mcrs_minecraft_nbt::COMPOUND_ID),
                ("nested.ids", mcrs_minecraft_nbt::LIST_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let mut nested = NbtCompound::new();
        nested.put_string("name", "mcrs".into());
        nested.put_list("ids", vec![NbtTag::Byte(1), NbtTag::Byte(2)]);
        let mut full = NbtCompound::new();
        full.put_byte("flag", 1);
        full.put_short("count", 300);
        full.put_int("big", 100_000);
        full.put_long("seed", 1234567890123);
        full.put_float("ratio", 0.5);
        full.put_double("precise", 0.1);
        full.put_component("nested", nested);
        vec![CustomData::default(), CustomData(full)]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BucketEntityData(pub CustomData);

impl Sample for BucketEntityData {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        self.0.nbt_tags()
    }

    fn samples() -> Vec<Self> {
        CustomData::samples()
            .into_iter()
            .map(BucketEntityData)
            .collect()
    }
}

/// ponytail: the persistent codec carries no registry, so an unknown
/// decoration type id is accepted here where vanilla fails the load; a
/// `DeserializeSeed` holding the lookup is the upgrade path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapDecoration {
    #[serde(rename = "type")]
    pub kind: ResourceKey<MapDecorationTypeReg>,
    pub x: f64,
    pub z: f64,
    pub rotation: f32,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapDecorations(pub BTreeMap<String, MapDecoration>);

impl Sample for MapDecorations {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.extend([
                ("m1", COMPOUND_ID),
                ("m1.type", STRING_ID),
                ("m1.x", DOUBLE_ID),
                ("m1.z", DOUBLE_ID),
                ("m1.rotation", FLOAT_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            MapDecorations::default(),
            MapDecorations(BTreeMap::from([(
                "m1".to_string(),
                MapDecoration {
                    kind: ResourceKey::from_location(ResourceLocation::minecraft("player")),
                    x: 1.5,
                    z: -2.5,
                    rotation: 90.0,
                },
            )])),
        ]
    }
}

/// ponytail: the block id and the property name are accepted as any strings
/// until a registry and block state definitions reach the persistent codec;
/// vanilla rejects an unknown block and a property the block does not have.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DebugStickState(pub BTreeMap<ResourceKey<BlockReg>, String>);

impl Sample for DebugStickState {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.push(("minecraft:oak_log", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            DebugStickState::default(),
            DebugStickState(BTreeMap::from([(
                ResourceKey::from_location(ResourceLocation::minecraft("oak_log")),
                "axis".to_string(),
            )])),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Recipes(pub Vec<ResourceKey<RecipeReg>>);

impl Sample for Recipes {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            Recipes::default(),
            Recipes(vec![
                ResourceKey::from_location(ResourceLocation::minecraft("stone")),
                ResourceKey::from_location(ResourceLocation::minecraft("oak_planks")),
            ]),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerLoot {
    pub loot_table: ResourceKey<LootTableReg>,
    #[serde(
        default,
        deserialize_with = "long_value",
        skip_serializing_if = "is_default"
    )]
    pub seed: i64,
}

impl Sample for ContainerLoot {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID), ("loot_table", STRING_ID)];
        if self.seed != 0 {
            tags.push(("seed", LONG_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let loot_table =
            ResourceKey::from_location(ResourceLocation::minecraft("chests/simple_dungeon"));
        vec![
            ContainerLoot {
                loot_table: loot_table.clone(),
                seed: 0,
            },
            ContainerLoot {
                loot_table,
                seed: 123456789012,
            },
        ]
    }
}
