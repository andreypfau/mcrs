use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_registries::{Named, Refs, RegistryOf, Resolved, registry_value};
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum ChunkGenerator<M: Refs = Resolved> {
    Noise(NoiseChunkGenerator<M>),
    Flat(FlatChunkGenerator),
    Debug,
}

mcrs_minecraft_registries::dispatch! {
    ChunkGenerator<Named>, key = "type", registry = mcrs_minecraft_dimension::keys::ChunkGeneratorType,
    {
        Noise => Noise,
        Flat => Flat,
        Debug => Debug,
    }
}

// chisle: the vanilla pack ships flat presets, so a flat generator must load, but nothing
// generates a flat world; its settings stay unread until a flat generator exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatChunkGenerator {
    pub settings: NbtTag,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseChunkGenerator<M: Refs = Resolved> {
    pub biome_source: BiomeSource<M>,
    pub settings: M::Id<RegistryOf<NoiseGeneratorSettings>>,
}
