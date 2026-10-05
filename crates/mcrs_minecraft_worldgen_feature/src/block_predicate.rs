use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::value_provider::VerticalAnchor;
use mcrs_minecraft_core::{codec::Validate, validated};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_worldgen_density::proto::BlockState;

use crate::placement::HeightmapName;

/// `Vec3i.offsetCodec(16)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Offset(pub [i32; 3]);

impl Offset {
    const LIMIT: i32 = 16;
}

impl Serialize for Offset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        IntArray(self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Offset {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let IntArray(axes) = IntArray::<3>::deserialize(deserializer)?;
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
        blocks: HolderSet<keys::Block>,
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
        fluids: HolderSet<keys::Fluid>,
    },
    #[serde(rename = "minecraft:matching_biomes")]
    MatchingBiomes { biomes: HolderSet<keys::Biome> },
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
    #[serde(rename = "minecraft:below_heightmap")]
    BelowHeightmap { heightmap: HeightmapName },
}

const BLOCK_PREDICATE_TYPE_ROWS: &[&str] = &[
    "minecraft:matching_blocks",
    "minecraft:matching_block_tag",
    "minecraft:matching_fluids",
    "minecraft:matching_biomes",
    "minecraft:has_sturdy_face",
    "minecraft:solid",
    "minecraft:replaceable",
    "minecraft:would_survive",
    "minecraft:inside_world_bounds",
    "minecraft:any_of",
    "minecraft:all_of",
    "minecraft:not",
    "minecraft:true",
    "minecraft:unobstructed",
    "minecraft:height_range",
    "minecraft:volume_match",
    "minecraft:below_heightmap",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    BLOCK_PREDICATE_TYPE_ROWS,
    &[],
    keys::block_predicate_type::NAMES
));

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
    use mcrs_minecraft_worldgen_testing::corpus_set;

    #[test]
    fn a_block_predicate_tag_resolves_in_the_loaded_scope() {
        let text =
            r##"{"type":"minecraft:matching_blocks","blocks":"#minecraft:base_stone_overworld"}"##;
        assert!(serde_json::from_str::<BlockPredicate>(text).is_err());

        let set = corpus_set();
        let tags = set.tags::<keys::Block>().unwrap();
        let BlockPredicate::MatchingBlocks {
            blocks: matching, ..
        } = set.scope(|| serde_json::from_str(text).unwrap())
        else {
            panic!("a matching_blocks predicate parses to its own variant");
        };
        assert!(matching.contains(keys::block::STONE, &tags));
        assert!(!matching.contains(keys::block::DIRT, &tags));
    }

    #[test]
    fn every_registered_predicate_type_is_a_variant() {
        let names = mcrs_minecraft_registry::static_report::shipped_report()
            .table("minecraft:block_predicate_type")
            .unwrap()
            .names();
        assert!(!names.is_empty());
        for name in names {
            let json = format!(r#"{{"type":"{name}"}}"#);
            if let Err(error) = serde_json::from_str::<BlockPredicate>(&json) {
                let error = error.to_string();
                assert!(!error.contains("unknown variant"), "{name}: {error}");
            }
        }
    }
}

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn block_predicate_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<BlockPredicate>(
            BLOCK_PREDICATE_TYPE_ROWS,
            &[],
            keys::block_predicate_type::NAMES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
