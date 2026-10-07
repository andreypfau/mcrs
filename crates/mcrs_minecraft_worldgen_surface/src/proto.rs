use serde::{Deserialize, Serialize};

use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_value_provider::VerticalAnchor;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_noise::proto::HashableF64;

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub enum MaterialRuleHolder {
    Reference(ResourceLocation),
    Owned(Box<MaterialRule>),
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub enum MaterialConditionHolder {
    Reference(ResourceLocation),
    Owned(Box<MaterialCondition>),
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self")]
#[serde(deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum MaterialRule {
    Block {
        result_state: BlockState,
    },
    Sequence {
        sequence: Vec<MaterialRuleHolder>,
    },
    Condition {
        if_true: MaterialConditionHolder,
        then_run: MaterialRuleHolder,
    },
    Bandlands,
    OreVein {
        ore_block: BlockState,
        raw_ore_block: BlockState,
        filler_block: BlockState,
        raw_ore_chance: HashableF64,
        density: DensityFunctionHolder,
        richness: DensityFunctionHolder,
        filler_gap: DensityFunctionHolder,
    },
}

mcrs_minecraft_registry::dispatch! {
    MaterialRule, key = "type", registry = crate::keys::MaterialRuleType,
    {
        Block => Block,
        Bandlands => Bandlands,
        Sequence => Sequence,
        Condition => Condition,
        OreVein => OreVein,
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self")]
#[serde(deny_unknown_fields)]
pub enum MaterialCondition {
    Biome {
        biome_is: HolderSet<mcrs_minecraft_biome::Biome>,
    },
    NoiseThreshold {
        noise: ResourceLocation,
        min_threshold: HashableF64,
        max_threshold: HashableF64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_3d: bool,
    },
    VerticalGradient {
        random_name: ResourceLocation,
        true_at_and_below: VerticalAnchor,
        false_at_and_above: VerticalAnchor,
    },
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Temperature,
    Steep,
    Not {
        invert: MaterialConditionHolder,
    },
    Hole,
    AbovePreliminarySurface,
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: CaveSurface,
    },
}

mcrs_minecraft_registry::dispatch! {
    MaterialCondition, key = "type", registry = crate::keys::MaterialConditionType,
    {
        Biome => Biome,
        NoiseThreshold => NoiseThreshold,
        VerticalGradient => VerticalGradient,
        YAbove => YAbove,
        Water => Water,
        Temperature => Temperature,
        Steep => Steep,
        Not => Not,
        Hole => Hole,
        AbovePreliminarySurface => AbovePreliminarySurface,
        StoneDepth => StoneDepth,
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveSurface {
    Ceiling,
    Floor,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::{corpus_set, round_trips};

    #[test]
    fn every_shipped_material_rule_and_condition_round_trips() {
        assert_eq!(round_trips::<MaterialRuleHolder>("material_rule"), 42);
        assert_eq!(
            round_trips::<MaterialConditionHolder>("material_condition"),
            8
        );
    }

    /// Neither shape occurs in the shipped corpus, so nothing else covers them.
    #[test]
    fn the_unshipped_shapes_round_trip() {
        let stated = r#"{"type":"minecraft:block","result_state":{"id":"minecraft:snow","properties":{"layers":"1"}}}"#;
        corpus_set().scope(|| {
            let rule: MaterialRule = serde_json::from_str(stated).unwrap();
            assert_eq!(serde_json::to_string(&rule).unwrap(), stated);

            for json in [
                r##"{"type":"minecraft:biome","biome_is":"#minecraft:is_overworld"}"##,
                r#"{"type":"minecraft:biome","biome_is":["minecraft:badlands","minecraft:eroded_badlands"]}"#,
            ] {
                let condition: MaterialCondition = serde_json::from_str(json).unwrap();
                assert_eq!(serde_json::to_string(&condition).unwrap(), json);
            }
        });
    }
}
