use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::block_state::BlockState;
use crate::state_properties::StatePropertiesPredicate;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{codec::Validate, validated};
use mcrs_minecraft_registries::{Named, Refs, Resolved, registry_value};
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

use mcrs_minecraft_core::Direction;

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum BlockPredicate<M: Refs = Resolved> {
    MatchingBlocks {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        blocks: M::Set<mcrs_minecraft_block::keys::Block>,
    },
    MatchingBlockTag {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        tag: ResourceLocation,
    },
    MatchingFluids {
        #[serde(default, skip_serializing_if = "is_default")]
        offset: Offset,
        fluids: M::Set<Fluid>,
        #[serde(default, skip_serializing_if = "StatePropertiesPredicate::is_any")]
        state: StatePropertiesPredicate,
    },
    MatchingBiomes {
        biomes: M::Set<Biome>,
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
        predicates: Vec<BlockPredicate<M>>,
    },
    AllOf {
        predicates: Vec<BlockPredicate<M>>,
    },
    Not {
        predicate: Box<BlockPredicate<M>>,
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
    VolumeMatch(VolumeMatch<M>),
    BelowHeightmap {
        heightmap: HeightmapName,
    },
}

mcrs_minecraft_registries::dispatch! {
    BlockPredicate<Named>, key = "type", registry = crate::keys::BlockPredicateType,
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

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct VolumeMatch<M: Refs = Resolved> {
    pub min: Offset,
    pub max: Offset,
    pub r#match: Box<BlockPredicate<M>>,
}

impl<M: Refs> Validate for VolumeMatch<M> {
    fn validate(&self) -> Result<(), String> {
        if (0..3).any(|axis| self.min.0[axis] > self.max.0[axis]) {
            return Err("min bound cannot be larger than max bound".to_owned());
        }
        Ok(())
    }
}

validated!(VolumeMatch<Named>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HeightmapName {
    WorldSurfaceWg,
    WorldSurface,
    OceanFloorWg,
    OceanFloor,
    MotionBlocking,
    MotionBlockingNoLeaves,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_nbt::{from_tag, to_nbt_tag};

    #[test]
    fn an_offset_is_an_int_array_in_nbt_and_a_list_in_json() {
        let offset = Offset([1, -2, 3]);

        assert_eq!(
            to_nbt_tag(&offset).unwrap(),
            NbtTag::IntArray(vec![1, -2, 3])
        );
        assert_eq!(serde_json::to_string(&offset).unwrap(), "[1,-2,3]");

        assert_eq!(
            from_tag::<Offset>(NbtTag::IntArray(vec![1, -2, 3])).unwrap(),
            offset
        );
        assert_eq!(serde_json::from_str::<Offset>("[1,-2,3]").unwrap(), offset);
    }

    #[test]
    fn a_matching_fluids_predicate_round_trips_with_and_without_state() {
        for text in [
            r#"{"type":"minecraft:matching_fluids","fluids":"minecraft:water"}"#,
            r#"{"type":"minecraft:matching_fluids","offset":[0,1,0],"fluids":"minecraft:water","state":{"source":"true"}}"#,
        ] {
            let written: BlockPredicate<Named> = serde_json::from_str(text).unwrap();
            assert_eq!(serde_json::to_string(&written).unwrap(), text, "{text}");
            let nbt = to_nbt_tag(&written).unwrap();
            assert_eq!(
                from_tag::<BlockPredicate<Named>>(nbt).unwrap(),
                written,
                "{text}"
            );
        }
    }

    #[test]
    fn every_registered_predicate_type_is_a_variant() {
        let names = crate::keys::BlockPredicateType::ENTRIES;
        assert!(!names.is_empty());
        for name in names {
            let json = format!(r#"{{"type":"{name}"}}"#);
            if let Err(error) = serde_json::from_str::<BlockPredicate<Named>>(&json) {
                let error = error.to_string();
                assert!(!error.contains("unknown variant"), "{name}: {error}");
            }
        }
    }
}
