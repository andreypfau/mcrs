use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceKey;
use serde::{Deserialize, Serialize};

use crate::dimension::DimensionEntry;
use mcrs_minecraft_dimension::Dimension;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldPreset {
    pub dimensions: BTreeMap<ResourceKey<Dimension>, DimensionEntry>,
}
