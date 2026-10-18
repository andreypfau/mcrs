use serde::{Deserialize, Serialize};

use super::flat::FlatChunkGenerator;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum ChunkGenerator {
    Noise(NoiseChunkGenerator),
    Flat(FlatChunkGenerator),
    Debug,
}

mcrs_minecraft_registry::dispatch! {
    ChunkGenerator, key = "type", registry = mcrs_minecraft_dimension::keys::ChunkGeneratorType,
    {
        Noise => Noise,
        Flat => Flat,
        Debug => Debug,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseChunkGenerator {
    pub biome_source: BiomeSource,
    #[serde(with = "mcrs_minecraft_registry::by_name")]
    pub settings: Id<NoiseGeneratorSettings>,
}
