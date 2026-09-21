use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, LIST_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::item::component::common::{BlockEntityTypeReg, EntityTypeReg, TypedEntityData};
use crate::item::harness::Sample;

// ponytail: `TypedEntityData<R>` derives `Clone`/`PartialEq`, which demands
// them of the uninhabited registry marker too; until that derive is replaced
// by bound-free impls, the wrappers here spell theirs out.
fn clone_typed<R>(data: &TypedEntityData<R>) -> TypedEntityData<R> {
    TypedEntityData {
        id: data.id.clone(),
        tag: data.tag.clone(),
    }
}

fn typed_eq<R>(a: &TypedEntityData<R>, b: &TypedEntityData<R>) -> bool {
    a.id == b.id && a.tag == b.tag
}

macro_rules! typed_entity_component {
    ($($ty:ident($registry:ident) [$first:literal, $second:literal]),* $(,)?) => {$(
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub TypedEntityData<$registry>);

        impl Clone for $ty {
            fn clone(&self) -> Self {
                $ty(clone_typed(&self.0))
            }
        }

        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                typed_eq(&self.0, &other.0)
            }
        }

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeeOccupant {
    pub entity_data: TypedEntityData<EntityTypeReg>,
    #[serde(deserialize_with = "int_value")]
    pub ticks_in_hive: i32,
    #[serde(deserialize_with = "int_value")]
    pub min_ticks_in_hive: i32,
}

impl Clone for BeeOccupant {
    fn clone(&self) -> Self {
        BeeOccupant {
            entity_data: clone_typed(&self.entity_data),
            ticks_in_hive: self.ticks_in_hive,
            min_ticks_in_hive: self.min_ticks_in_hive,
        }
    }
}

impl PartialEq for BeeOccupant {
    fn eq(&self, other: &Self) -> bool {
        typed_eq(&self.entity_data, &other.entity_data)
            && self.ticks_in_hive == other.ticks_in_hive
            && self.min_ticks_in_hive == other.min_ticks_in_hive
    }
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
