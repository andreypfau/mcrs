use std::fmt;
use std::marker::PhantomData;

use serde::de::Error as _;
use serde::de::{MapAccess, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::tree::is_default;
use crate::proto::{BlockState, Validate, validated};
use crate::value_provider::VerticalAnchor;
use mcrs_minecraft_core::ResourceLocation;

/// A registry element set as `RegistryCodecs.holderSet` writes it: a `#tag`,
/// one entry, or a list of entries. The three shapes are kept apart so a value
/// serializes back the way it came. With `ALWAYS_LIST` — the codec's
/// `alwaysUseList` — a bare entry is refused and only a tag or a list reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HolderSet<T = ResourceLocation, const ALWAYS_LIST: bool = false> {
    Tag(ResourceLocation),
    One(T),
    List(Vec<T>),
}

impl<T, const ALWAYS_LIST: bool> HolderSet<T, ALWAYS_LIST> {
    /// The entries a tag-less set names; a tag names none until it is expanded.
    pub fn entries(&self) -> &[T] {
        match self {
            HolderSet::Tag(_) => &[],
            HolderSet::One(entry) => std::slice::from_ref(entry),
            HolderSet::List(entries) => entries,
        }
    }
}

impl<'de, T: Deserialize<'de>, const ALWAYS_LIST: bool> Deserialize<'de>
    for HolderSet<T, ALWAYS_LIST>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor<T, const ALWAYS_LIST: bool>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>, const ALWAYS_LIST: bool> Visitor<'de>
            for SetVisitor<T, ALWAYS_LIST>
        {
            type Value = HolderSet<T, ALWAYS_LIST>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(if ALWAYS_LIST {
                    "a tag or a list of entries"
                } else {
                    "a tag, an entry, or a list of entries"
                })
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if let Some(tag) = text.strip_prefix('#') {
                    return ResourceLocation::parse(tag)
                        .map(HolderSet::Tag)
                        .map_err(E::custom);
                }
                if ALWAYS_LIST {
                    return Err(E::custom(format!("Not a tag id: {text}")));
                }
                T::deserialize(value::StrDeserializer::new(text)).map(HolderSet::One)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                if ALWAYS_LIST {
                    return Err(A::Error::custom("Not a tag id: an inline entry"));
                }
                T::deserialize(value::MapAccessDeserializer::new(map)).map(HolderSet::One)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(HolderSet::List)
            }
        }

        deserializer.deserialize_any(SetVisitor(PhantomData))
    }
}

impl<T: Serialize, const ALWAYS_LIST: bool> Serialize for HolderSet<T, ALWAYS_LIST> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            HolderSet::Tag(tag) => serializer.serialize_str(&format!("#{}", tag.as_str())),
            HolderSet::One(entry) => entry.serialize(serializer),
            HolderSet::List(entries) => entries.serialize(serializer),
        }
    }
}

/// `Vec3i.offsetCodec(16)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize)]
#[serde(transparent)]
pub struct Offset(pub [i32; 3]);

impl Offset {
    const LIMIT: i32 = 16;
}

impl<'de> Deserialize<'de> for Offset {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let axes = <[i32; 3]>::deserialize(deserializer)?;
        if let Some(out) = axes.iter().find(|a| a.abs() > Offset::LIMIT) {
            return Err(D::Error::custom(format!(
                "offset {out} is out of range, expected at most {}",
                Offset::LIMIT
            )));
        }
        Ok(Offset(axes))
    }
}

pub use mcrs_minecraft_core::Direction;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockPredicate {
    #[serde(rename = "minecraft:matching_blocks")]
    MatchingBlocks {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        blocks: HolderSet,
    },
    #[serde(rename = "minecraft:matching_block_tag")]
    MatchingBlockTag {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        tag: ResourceLocation,
    },
    #[serde(rename = "minecraft:matching_fluids")]
    MatchingFluids {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        fluids: HolderSet,
    },
    #[serde(rename = "minecraft:matching_biomes")]
    MatchingBiomes { biomes: HolderSet },
    #[serde(rename = "minecraft:has_sturdy_face")]
    HasSturdyFace {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        direction: Direction,
    },
    #[serde(rename = "minecraft:solid")]
    Solid {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    #[serde(rename = "minecraft:replaceable")]
    Replaceable {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    #[serde(rename = "minecraft:would_survive")]
    WouldSurvive {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        state: BlockState,
    },
    #[serde(rename = "minecraft:inside_world_bounds")]
    InsideWorldBounds {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:all_of")]
    AllOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:not")]
    Not { predicate: Box<BlockPredicate> },
    #[serde(rename = "minecraft:true")]
    True,
    /// The one offset the reference does not bound to sixteen blocks per axis.
    #[serde(rename = "minecraft:unobstructed")]
    Unobstructed {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: [i32; 3],
    },
    #[serde(rename = "minecraft:height_range")]
    HeightRange {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    #[serde(rename = "minecraft:volume_match")]
    VolumeMatch(VolumeMatch),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct VolumeMatch {
    pub min: Offset,
    pub max: Offset,
    pub r#match: Box<BlockPredicate>,
}

impl Validate for VolumeMatch {
    fn validate(&self) -> Result<(), String> {
        if (0..3).any(|axis| self.min.0[axis] > self.max.0[axis]) {
            return Err("min bound cannot be larger than max bound".to_owned());
        }
        Ok(())
    }
}

validated!(VolumeMatch);

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(json: &str) {
        let parsed: BlockPredicate = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn every_shipped_shape_round_trips() {
        round_trip(
            r#"{"type":"minecraft:matching_blocks","blocks":["minecraft:dirt","minecraft:mud"]}"#,
        );
        round_trip(
            r#"{"type":"minecraft:matching_blocks","offset":[0,1,0],"blocks":"minecraft:water"}"#,
        );
        round_trip(r#"{"type":"minecraft:matching_block_tag","tag":"minecraft:air"}"#);
        round_trip(
            r#"{"type":"minecraft:matching_fluids","offset":[0,1,0],"fluids":"minecraft:water"}"#,
        );
        round_trip(r##"{"type":"minecraft:matching_biomes","biomes":"#minecraft:is_jungle"}"##);
        round_trip(r#"{"type":"minecraft:has_sturdy_face","offset":[0,-1,0],"direction":"up"}"#);
        round_trip(r#"{"type":"minecraft:solid"}"#);
        round_trip(r#"{"type":"minecraft:solid","offset":[0,1,0]}"#);
        round_trip(r#"{"type":"minecraft:replaceable"}"#);
        round_trip(r#"{"type":"minecraft:would_survive","state":"minecraft:oak_sapling"}"#);
        round_trip(r#"{"type":"minecraft:inside_world_bounds","offset":[0,-5,0]}"#);
        round_trip(r#"{"type":"minecraft:true"}"#);
        round_trip(r#"{"type":"minecraft:unobstructed"}"#);
        round_trip(
            r#"{"type":"minecraft:height_range","min_inclusive":{"above_bottom":0},"max_inclusive":{"relative_to_sea_level":0}}"#,
        );
        round_trip(
            r#"{"type":"minecraft:not","predicate":{"type":"minecraft:matching_block_tag","tag":"minecraft:air"}}"#,
        );
        round_trip(
            r#"{"type":"minecraft:any_of","predicates":[{"type":"minecraft:solid"},{"type":"minecraft:true"}]}"#,
        );
        round_trip(r#"{"type":"minecraft:all_of","predicates":[{"type":"minecraft:solid"}]}"#);
        round_trip(
            r#"{"type":"minecraft:volume_match","min":[-2,-2,-2],"max":[2,-1,2],"match":{"type":"minecraft:true"}}"#,
        );
    }

    #[test]
    fn an_out_of_range_offset_is_a_load_error() {
        let error = serde_json::from_str::<BlockPredicate>(
            r#"{"type":"minecraft:solid","offset":[0,17,0]}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("out of range"), "{error}");
    }

    #[test]
    fn an_inverted_volume_is_a_load_error() {
        let error = serde_json::from_str::<BlockPredicate>(
            r#"{"type":"minecraft:volume_match","min":[0,0,0],"max":[0,-1,0],"match":{"type":"minecraft:true"}}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("min bound cannot be larger"), "{error}");
    }

    #[test]
    fn an_unregistered_type_is_a_load_error() {
        let error = serde_json::from_str::<BlockPredicate>(r#"{"type":"minecraft:has_water"}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("minecraft:has_water"), "{error}");
    }
}
