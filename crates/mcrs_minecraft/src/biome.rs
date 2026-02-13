use crate::sound::{Music, SoundEvent};
use crate::weight::Weighted;
use mcrs_registry::Holder;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Biome {
    #[serde(flatten)]
    pub climate_settings: ClimateSettings,
    pub effects: BiomeSpecialEffects,
}

#[derive(Default, Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClimateSettings {
    pub has_precipitation: bool,
    pub temperature: f32,
    #[serde(default)]
    pub temperature_modifier: TemperatureModifier,
    pub downfall: f32,
}

#[derive(Default, Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TemperatureModifier {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "frozen")]
    Frozen,
}

#[derive(Default, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BiomeSpecialEffects {
    pub fog_color: u32,
    pub water_color: u32,
    pub water_fog_color: u32,
    pub sky_color: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foliage_color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_foliage_color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grass_color: Option<u32>,
    #[serde(default)]
    pub grass_color_modifier: GrassColorModifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub particle: Option<Particle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambient_sound: Option<Holder<SoundEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood_sound: Option<AmbientMoodSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additions_sound: Option<AmbientAdditionsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<Vec<Weighted<Music>>>,
    pub music_volume: f32,
}

#[derive(Default, Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum GrassColorModifier {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "dark_forest")]
    DarkForest,
    #[serde(rename = "swamp")]
    Swamp,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Particle {
    pub options: ParticleOptions,
    pub probability: f32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParticleOptions {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AmbientMoodSettings {
    pub sound: Holder<SoundEvent>,
    pub tick_delay: i32,
    pub block_search_extent: i32,
    pub offset: f64,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AmbientAdditionsSettings {
    pub sound: Holder<SoundEvent>,
    pub tick_chance: f64,
}

pub struct GenerationSettings {}

pub struct MobSpawnSettings {}
