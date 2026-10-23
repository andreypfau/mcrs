use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_registries::{Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::dimension::DimensionEntry;
use mcrs_minecraft_dimension::Dimension;

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldPreset<M: Refs = Resolved> {
    pub dimensions: BTreeMap<ResourceKey<Dimension>, DimensionEntry<M>>,
}
