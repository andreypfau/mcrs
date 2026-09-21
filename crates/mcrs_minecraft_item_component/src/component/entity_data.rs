use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, LIST_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::component::common::{BlockEntityTypeReg, EntityTypeReg, TypedEntityData};
use crate::harness::Sample;

macro_rules! typed_entity_component {
    ($($ty:ident($registry:ident) [$first:literal, $second:literal]),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub TypedEntityData<$registry>);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                typed_tags(&self.0.tag)
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty(typed($first, NbtCompound::new())),
                    $ty(typed($second, sample_tag())),
                ]
            }
        }
    )*};
}

typed_entity_component! {
    EntityData(EntityTypeReg) ["zombie", "pig"],
    BlockEntityData(BlockEntityTypeReg) ["chest", "sign"],
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bees(pub Vec<BeeOccupant>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeeOccupant {
    pub entity_data: TypedEntityData<EntityTypeReg>,
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
                    entity_data: typed("pig", sample_tag()),
                    ticks_in_hive: 10,
                    min_ticks_in_hive: 600,
                },
                BeeOccupant {
                    entity_data: typed("zombie", NbtCompound::new()),
                    ticks_in_hive: 0,
                    min_ticks_in_hive: -1,
                },
            ]),
        ]
    }
}

fn typed<R>(path: &str, tag: NbtCompound) -> TypedEntityData<R> {
    TypedEntityData {
        id: ResourceKey::from_location(ResourceLocation::minecraft(path)),
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
