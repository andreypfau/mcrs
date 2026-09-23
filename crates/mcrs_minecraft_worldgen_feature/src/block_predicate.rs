use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::value_provider::VerticalAnchor;
use mcrs_minecraft_core::{codec::Validate, validated};
use mcrs_minecraft_worldgen_density::proto::BlockState;

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
