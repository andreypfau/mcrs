use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;
use serde::{Deserialize, Serialize};

use crate::dimension::DimensionEntry;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldPreset {
    pub dimensions: BTreeMap<ResourceKey<keys::Dimension>, DimensionEntry>,
}
