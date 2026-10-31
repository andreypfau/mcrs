use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_registries::{Refs, Resolved, dispatch, registry_value};

#[registry_value]
#[dispatch(registry = mcrs_minecraft_dimension::keys::ChunkGeneratorType)]
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkGenerator<M: Refs = Resolved> {
    Noise(NoiseChunkGenerator<M>),
    Flat(FlatChunkGenerator),
    Debug,
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
    pub settings: M::Id<mcrs_minecraft_dimension::NoiseSettings>,
}
