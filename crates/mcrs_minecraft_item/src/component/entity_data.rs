use mcrs_minecraft_block::keys::BlockEntityType;
use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_resource::{ResourceKeyBuf, StaticResourceLocation, rl};
use serde::{Deserialize, Serialize};

use crate::component::common::TypedEntityData;
use crate::harness::Sample;
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt};
use std::io::Write;

macro_rules! typed_entity_component {
    ($($ty:ident($registry:ident) [$first:literal, $second:literal]),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)]
        #[serde(transparent)]
        pub struct $ty(pub TypedEntityData<$registry>);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                typed_tags(&self.0.tag)
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty(typed(rl!($first), NbtCompound::new())),
                    $ty(typed(rl!($second), sample_tag())),
                ]
            }
        }
    )*};
}

typed_entity_component! {
    EntityData(EntityType) ["minecraft:zombie", "minecraft:pig"],
    BlockEntityData(BlockEntityType) ["minecraft:chest", "minecraft:sign"],
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, Encode, Decode)]
#[serde(transparent)]
pub struct Bees(pub Vec<BeeOccupant>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeeOccupant {
    pub entity_data: TypedEntityData<EntityType>,
    #[serde(deserialize_with = "int_value")]
    pub ticks_in_hive: i32,
    #[serde(deserialize_with = "int_value")]
    pub min_ticks_in_hive: i32,
}

impl Sample for Bees {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            Bees::default(),
            Bees(vec![
                BeeOccupant {
                    entity_data: typed(rl!("minecraft:pig"), sample_tag()),
                    ticks_in_hive: 10,
                    min_ticks_in_hive: 600,
                },
                BeeOccupant {
                    entity_data: typed(rl!("minecraft:zombie"), NbtCompound::new()),
                    ticks_in_hive: 0,
                    min_ticks_in_hive: -1,
                },
            ]),
        ]
    }
}

fn typed<R>(id: StaticResourceLocation, tag: NbtCompound) -> TypedEntityData<R> {
    TypedEntityData {
        id: ResourceKeyBuf::from_location(id.to_buf()),
        tag,
    }
}

fn sample_tag() -> NbtCompound {
    let mut tag = NbtCompound::new();
    tag.put_byte("Health", 20);
    tag.put_bool("IsBaby", true);
    tag.put_string("CustomName", "bee".into());
    tag
}

fn typed_tags(tag: &NbtCompound) -> Vec<(&'static str, u8)> {
    let mut tags = vec![("", COMPOUND_ID), ("id", STRING_ID)];
    if !tag.is_empty() {
        tags.extend([
            ("Health", BYTE_ID),
            ("IsBaby", BYTE_ID),
            ("CustomName", STRING_ID),
        ]);
    }
    tags
}

impl Encode for BeeOccupant {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.entity_data.encode(&mut w)?;
        VarInt(self.ticks_in_hive).encode(&mut w)?;
        VarInt(self.min_ticks_in_hive).encode(w)
    }
}

impl Decode<'_> for BeeOccupant {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(BeeOccupant {
            entity_data: TypedEntityData::decode(r)?,
            ticks_in_hive: VarInt::decode(r)?.0,
            min_ticks_in_hive: VarInt::decode(r)?.0,
        })
    }
}
