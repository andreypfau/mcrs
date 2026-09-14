use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::value_provider::VerticalAnchor;
use mcrs_minecraft_worldgen_density::proto::{BlockState, DensityFunctionHolder};
use mcrs_minecraft_worldgen_noise::proto::HashableF64;

fn is_false(value: &bool) -> bool {
    !*value
}

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
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum MaterialRule {
    #[serde(rename = "minecraft:block")]
    Block { result_state: BlockState },
    #[serde(rename = "minecraft:sequence")]
    Sequence { sequence: Vec<MaterialRuleHolder> },
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: MaterialConditionHolder,
        then_run: MaterialRuleHolder,
    },
    #[serde(rename = "minecraft:bandlands")]
    Bandlands,
    #[serde(rename = "minecraft:ore_vein")]
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

#[derive(Hash, Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum MaterialCondition {
    #[serde(rename = "minecraft:biome")]
    Biome { biome_is: BiomeSet },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: ResourceLocation,
        min_threshold: HashableF64,
        max_threshold: HashableF64,
        #[serde(default, skip_serializing_if = "is_false")]
        is_3d: bool,
    },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: ResourceLocation,
        true_at_and_below: VerticalAnchor,
        false_at_and_above: VerticalAnchor,
    },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:temperature")]
    Temperature,
    #[serde(rename = "minecraft:steep")]
    Steep,
    #[serde(rename = "minecraft:not")]
    Not { invert: MaterialConditionHolder },
    #[serde(rename = "minecraft:hole")]
    Hole,
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface,
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: CaveSurface,
    },
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveSurface {
    Ceiling,
    Floor,
}

/// The biomes a `biome_is` names: one id or a list of them. A `#tag` parses
/// but the material compile refuses it.
pub type BiomeSet = mcrs_minecraft_core::HolderSet;

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::round_trips;

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
    fn a_stated_result_state_and_a_biome_tag_round_trip() {
        let stated = r#"{"type":"minecraft:block","result_state":{"id":"minecraft:snow","properties":{"layers":"1"}}}"#;
        let rule: MaterialRule = serde_json::from_str(stated).unwrap();
        assert_eq!(serde_json::to_string(&rule).unwrap(), stated);

        let tagged = r##"{"type":"minecraft:biome","biome_is":"#minecraft:is_overworld"}"##;
        let condition: MaterialCondition = serde_json::from_str(tagged).unwrap();
        assert!(
            matches!(&condition, MaterialCondition::Biome { biome_is } if matches!(biome_is, BiomeSet::Tag(_)))
        );
        assert_eq!(serde_json::to_string(&condition).unwrap(), tagged);
    }

    #[test]
    fn a_biome_list_keeps_its_shape() {
        let listed = r#"{"type":"minecraft:biome","biome_is":["minecraft:badlands","minecraft:eroded_badlands"]}"#;
        let condition: MaterialCondition = serde_json::from_str(listed).unwrap();
        assert_eq!(serde_json::to_string(&condition).unwrap(), listed);
    }
}
