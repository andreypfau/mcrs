use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize};

use crate::item::component::common::{compound_or_snbt, stub_component};
use crate::item::ctx::ctx_free;
use crate::item::harness::Sample;
use crate::{Decode, Encode};

/// `CustomData.CODEC`: the compound as is; an SNBT string reads as one too.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Encode, Decode)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
#[serde(transparent)]
pub struct CustomData(pub NbtCompound);

impl<'de> Deserialize<'de> for CustomData {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        compound_or_snbt(d).map(CustomData)
    }
}

ctx_free!(CustomData);

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

stub_component!(
    MapDecorations,
    DebugStickState,
    BucketEntityData,
    Recipes,
    ContainerLoot,
);
