use serde::{Deserialize, Serialize};

use super::flat::FlatChunkGenerator;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_registry::Id;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
#[allow(clippy::large_enum_variant)]
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
    pub settings: Id<NoiseGeneratorSettings>,
}
