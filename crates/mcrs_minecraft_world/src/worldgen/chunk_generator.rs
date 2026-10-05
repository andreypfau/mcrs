use serde::{Deserialize, Serialize};

use super::flat::FlatChunkGenerator;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::Id;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum ChunkGenerator {
    #[serde(rename = "minecraft:noise")]
    Noise(NoiseChunkGenerator),
    #[serde(rename = "minecraft:flat")]
    Flat(FlatChunkGenerator),
    #[serde(rename = "minecraft:debug")]
    Debug,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseChunkGenerator {
    pub biome_source: BiomeSource,
    pub settings: Id<keys::NoiseSettings>,
}
