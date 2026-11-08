use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_registries::{Refs, Resolved, dispatch, registry_value};

#[registry_value]
#[dispatch(registry = mcrs_minecraft_dimension::chunk_generator_keys::ChunkGeneratorType)]
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkGenerator<M: Refs = Resolved> {
    Noise(NoiseChunkGenerator<M>),
    Flat(FlatChunkGenerator),
    Debug,
}

/// The settings of a flat generator stay NBT: the world generation reads only their layer list
/// and their biome.
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
