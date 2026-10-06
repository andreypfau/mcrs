pub mod climate;
pub mod overworld_preset;
pub mod parameter_list;
pub mod source;
pub mod zoom;

use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::codec::{HexRgb, is_default};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureModifier {
    None,
    Frozen,
}

/// The part of a biome every crate that names one can hold: its climate and its
/// colours. The environment attributes and the generation settings are columns
/// beside the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct Biome {
    pub temperature: f32,
    pub downfall: f32,
    pub has_precipitation: bool,
    pub temperature_modifier: Option<TemperatureModifier>,
    pub effects: BiomeEffects,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrassColorModifier {
    #[default]
    None,
    DarkForest,
    Swamp,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BiomeEffects {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foliage_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grass_color: Option<HexRgb>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub grass_color_modifier: GrassColorModifier,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_foliage_color: Option<HexRgb>,
}
