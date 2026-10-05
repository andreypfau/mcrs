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

const CHUNK_GENERATOR_ROWS: &[&str] = &["minecraft:noise", "minecraft:flat", "minecraft:debug"];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    CHUNK_GENERATOR_ROWS,
    &[],
    keys::chunk_generator::NAMES
));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseChunkGenerator {
    pub biome_source: BiomeSource,
    pub settings: Id<keys::NoiseSettings>,
}

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn chunk_generator_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<ChunkGenerator>(
            CHUNK_GENERATOR_ROWS,
            &[],
            keys::chunk_generator::NAMES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
