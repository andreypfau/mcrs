use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block_state::BlockState;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{codec::Validate, validated};
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_value_provider::VerticalAnchor;

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
#[serde(remote = "Self", deny_unknown_fields)]
pub enum BlockPredicate {
    MatchingBlocks {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        blocks: HolderSet<mcrs_minecraft_block::keys::Block>,
    },
    MatchingBlockTag {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        tag: ResourceLocation,
    },
    MatchingFluids {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        fluids: HolderSet<Fluid>,
    },
    MatchingBiomes {
        biomes: HolderSet<Biome>,
    },
    HasSturdyFace {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        direction: Direction,
    },
    Solid {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    Replaceable {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    WouldSurvive {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        state: BlockState,
    },
    InsideWorldBounds {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
    },
    AnyOf {
        predicates: Vec<BlockPredicate>,
    },
    AllOf {
        predicates: Vec<BlockPredicate>,
    },
    Not {
        predicate: Box<BlockPredicate>,
    },
    True,
    /// The one offset the reference does not bound to sixteen blocks per axis.
    Unobstructed {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: [i32; 3],
    },
    HeightRange {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    VolumeMatch(VolumeMatch),
    BelowHeightmap {
        heightmap: HeightmapName,
    },
}

mcrs_minecraft_registry::dispatch! {
    BlockPredicate, key = "type", registry = crate::keys::BlockPredicateType,
    {
        MatchingBlocks => MatchingBlocks,
        MatchingBlockTag => MatchingBlockTag,
        MatchingFluids => MatchingFluids,
        MatchingBiomes => MatchingBiomes,
        HasSturdyFace => HasSturdyFace,
        Solid => Solid,
        Replaceable => Replaceable,
        WouldSurvive => WouldSurvive,
        InsideWorldBounds => InsideWorldBounds,
        AnyOf => AnyOf,
        AllOf => AllOf,
        Not => Not,
        True => True,
        Unobstructed => Unobstructed,
        HeightRange => HeightRange,
        VolumeMatch => VolumeMatch,
        BelowHeightmap => BelowHeightmap,
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightmapName {
    #[serde(rename = "WORLD_SURFACE_WG")]
    WorldSurfaceWg,
    #[serde(rename = "WORLD_SURFACE")]
    WorldSurface,
    #[serde(rename = "OCEAN_FLOOR_WG")]
    OceanFloorWg,
    #[serde(rename = "OCEAN_FLOOR")]
    OceanFloor,
    #[serde(rename = "MOTION_BLOCKING")]
    MotionBlocking,
    #[serde(rename = "MOTION_BLOCKING_NO_LEAVES")]
    MotionBlockingNoLeaves,
}

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
        let tags = set.tags::<mcrs_minecraft_block::keys::Block>().unwrap();
        let BlockPredicate::MatchingBlocks {
            blocks: matching, ..
        } = set.scope(|| serde_json::from_str(text).unwrap())
        else {
            panic!("a matching_blocks predicate parses to its own variant");
        };
        assert!(matching.contains(mcrs_minecraft_block::keys::Block::Stone.id(), &tags));
        assert!(!matching.contains(mcrs_minecraft_block::keys::Block::Dirt.id(), &tags));
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
