use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::proto::{BlockState, DensityFunctionHolder, Either, HashableF64};
use crate::value_provider::VerticalAnchor;
use mcrs_minecraft_core::ResourceLocation;

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

/// The biomes a `biome_is` names, written either as one bare id or as a list of
/// them, and written back the way it came.
#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum BiomeSet {
    One(ResourceLocation),
    Many(Vec<ResourceLocation>),
}

impl BiomeSet {
    pub fn ids(&self) -> &[ResourceLocation] {
        match self {
            BiomeSet::One(id) => std::slice::from_ref(id),
            BiomeSet::Many(ids) => ids,
        }
    }
}

impl<'de> Deserialize<'de> for BiomeSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let set =
            match Either::<ResourceLocation, Vec<ResourceLocation>>::deserialize(deserializer)? {
                Either::Left(id) => BiomeSet::One(id),
                Either::Right(ids) => BiomeSet::Many(ids),
            };
        // A tag parses as an ordinary id here — `#minecraft:is_overworld` splits
        // into namespace `#minecraft` — so it has to be refused by name.
        if let Some(tag) = set.ids().iter().find(|id| id.as_str().starts_with('#')) {
            return Err(D::Error::custom(format!(
                "biome tag {tag} is not supported in biome_is"
            )));
        }
        Ok(set)
    }
}

impl Serialize for BiomeSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            BiomeSet::One(id) => id.serialize(serializer),
            BiomeSet::Many(ids) => ids.serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn worldgen() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
    }

    fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                json_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                out.push(path);
            }
        }
    }

    fn round_trips<T: serde::de::DeserializeOwned + Serialize>(directory: &str) -> usize {
        let mut paths = Vec::new();
        json_files(&worldgen().join(directory), &mut paths);
        for path in &paths {
            let bytes = std::fs::read(path).unwrap();
            let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let parsed: T = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                serde_json::to_value(&parsed).unwrap(),
                raw,
                "{} does not round-trip",
                path.display()
            );
        }
        paths.len()
    }

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
    fn a_stated_result_state_round_trips_and_a_biome_tag_is_refused() {
        let stated = r#"{"type":"minecraft:block","result_state":{"id":"minecraft:snow","properties":{"layers":"1"}}}"#;
        let rule: MaterialRule = serde_json::from_str(stated).unwrap();
        assert_eq!(serde_json::to_string(&rule).unwrap(), stated);

        let error = serde_json::from_str::<MaterialCondition>(
            r##"{"type":"minecraft:biome","biome_is":"#minecraft:is_overworld"}"##,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("is not supported"), "{error}");
    }

    #[test]
    fn a_biome_list_keeps_its_shape() {
        let listed = r#"{"type":"minecraft:biome","biome_is":["minecraft:badlands","minecraft:eroded_badlands"]}"#;
        let condition: MaterialCondition = serde_json::from_str(listed).unwrap();
        assert_eq!(serde_json::to_string(&condition).unwrap(), listed);
    }
}
