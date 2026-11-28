use std::collections::BTreeMap;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{COMPOUND_ID, DOUBLE_ID, FLOAT_ID, LIST_ID, LONG_ID, STRING_ID};
use mcrs_minecraft_resource::{ResourceKeyBuf, rl};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::common::compound_or_snbt;
use crate::harness::Sample;
use crate::recipe::Recipe;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_map_decoration::map_decoration_type_keys::MapDecorationType;
use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Encode, Decode)]
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

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, Encode, Decode)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapDecoration {
    #[serde(rename = "type")]
    pub kind: ResourceKeyBuf<MapDecorationType>,
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
                    kind: ResourceKeyBuf::from_location(
                        mcrs_minecraft_map_decoration::map_decoration_type_keys::MapDecorationType::Player
                            .location()
                            .to_buf(),
                    ),
                    x: 1.5,
                    z: -2.5,
                    rotation: 90.0,
                },
            )])),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DebugStickState(pub BTreeMap<ResourceKeyBuf<Block>, String>);

impl Sample for DebugStickState {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if !self.0.is_empty() {
            tags.push((Block::OakLog.as_static_str(), STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            DebugStickState::default(),
            DebugStickState(BTreeMap::from([(
                ResourceKeyBuf::from_location(Block::OakLog.location().to_buf()),
                "axis".to_string(),
            )])),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Recipes(pub Vec<ResourceKeyBuf<Recipe>>);

impl Sample for Recipes {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            Recipes::default(),
            Recipes(vec![
                ResourceKeyBuf::from_location(rl!("minecraft:stone").to_buf()),
                ResourceKeyBuf::from_location(rl!("minecraft:oak_planks").to_buf()),
            ]),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerLoot {
    pub loot_table: ResourceKeyBuf<LootTable>,
    #[serde(
        default,
        deserialize_with = "seed_value",
        skip_serializing_if = "is_default"
    )]
    pub seed: i64,
}

fn seed_value<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    #[derive(PartialEq)]
    enum Seed {
        Json,
        Nbt,
    }

    impl Visitor<'_> for Seed {
        type Value = i64;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number")
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<i64, E> {
            Ok(v.cast_signed())
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<i64, E> {
            let truncated = v.trunc();
            Ok(
                if self == Seed::Json && (-(2f64.powi(63))..2f64.powi(64)).contains(&truncated) {
                    truncated as i128 as i64
                } else {
                    truncated as i64
                },
            )
        }
    }

    let seed = if d.is_human_readable() {
        Seed::Json
    } else {
        Seed::Nbt
    };
    d.deserialize_any(seed)
}

#[derive(Serialize)]
struct ContainerLootWire<'a> {
    #[serde(skip_serializing_if = "is_default")]
    seed: i64,
    loot_table: &'a ResourceKeyBuf<LootTable>,
}

impl Encode for ContainerLoot {
    fn encode(&self, w: impl Write) -> Result<()> {
        let wire = ContainerLootWire {
            seed: self.seed,
            loot_table: &self.loot_table,
        };
        mcrs_minecraft_nbt::encode_wire(&wire, w)
    }
}

impl Decode<'_> for ContainerLoot {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        mcrs_minecraft_nbt::decode_wire(r)
    }
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
        let loot_table: ResourceKeyBuf<LootTable> =
            mcrs_minecraft_loot::loot_table_keys::CHESTS_SIMPLE_DUNGEON.into();
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

macro_rules! nbt_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl mcrs_minecraft_wire::Encode for $ty {
            fn encode(&self, w: impl Write) -> Result<()> {
                mcrs_minecraft_nbt::encode_wire(self, w)
            }
        }

        impl mcrs_minecraft_wire::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> Result<Self> {
                mcrs_minecraft_nbt::decode_wire(r)
            }
        }
    )*};
}

nbt_wire!(MapDecorations, DebugStickState, Recipes);
