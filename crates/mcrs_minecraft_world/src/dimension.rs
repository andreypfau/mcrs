use serde::{Deserialize, Serialize};

use crate::worldgen::chunk_generator::ChunkGenerator;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::Id;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionEntry {
    #[serde(rename = "type")]
    pub dimension_type: Id<keys::DimensionType>,
    pub generator: ChunkGenerator,
}
