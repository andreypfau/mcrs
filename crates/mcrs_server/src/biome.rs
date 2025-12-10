use crate::sound::{Music, SoundEvent};
use crate::weight::Weighted;
use mcrs_protocol::sound::SoundId;
use mcrs_registry::{Holder, RegistryRef};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
struct Biome {
    #[serde(flatten)]
    climate_settings: ClimateSettings,
    effects: BiomeSpecialEffects,
}

#[derive(Default, Copy, Clone, serde::Deserialize, serde::Serialize)]
struct ClimateSettings {
    has_precipitation: bool,
    temperature: f32,
    #[serde(default)]
    temperature_modifier: TemperatureModifier,
    downfall: f32,
}

#[derive(Default, Copy, Clone, serde::Deserialize, serde::Serialize)]
enum TemperatureModifier {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "frozen")]
    Frozen,
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
struct BiomeSpecialEffects {
    fog_color: u32,
    water_color: u32,
    water_fog_color: u32,
    sky_color: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    foliage_color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dry_foliage_color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grass_color: Option<u32>,
    #[serde(default)]
    grass_color_modifier: GrassColorModifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    particle: Option<Particle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ambient_sound: Option<Holder<SoundEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mood_sound: Option<AmbientMoodSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    additions_sound: Option<AmbientAdditionsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    music: Option<Vec<Weighted<Music>>>,
    music_volume: f32,
}

#[derive(Default, Copy, Clone, serde::Deserialize, serde::Serialize)]
enum GrassColorModifier {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "dark_forest")]
    DarkForest,
    #[serde(rename = "swamp")]
    Swamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Particle {
    options: ParticleOptions,
    portability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParticleOptions {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AmbientMoodSettings {
    sound: Holder<SoundEvent>,
    tick_delay: i32,
    block_search_extent: i32,
    offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AmbientAdditionsSettings {
    sound: Holder<SoundEvent>,
    tick_chance: f64,
}

struct GenerationSettings {

}

struct MobSpawnSettings {

}
