use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_structure::{MobCategory, SpawnerData};
use serde::{Deserialize, Serialize};

/// The `minecraft:gameplay/natural_mob_spawns` attribute argument.
///
/// A category absent from `spawns_by_category` is undefined and falls through
/// to the layer below; a category present but empty suppresses it, which is how
/// `deep_dark` and `the_void` silence the dimension's spawns.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobSpawnSettings {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawn_costs: BTreeMap<ResourceLocation<Arc<str>>, SpawnCost>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spawns_by_category: BTreeMap<MobCategory, Vec<SpawnerData>>,
}

impl MobSpawnSettings {
    pub fn is_empty(&self) -> bool {
        self.spawn_costs.is_empty() && self.spawns_by_category.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnCost {
    pub charge: f64,
    pub energy_budget: f64,
}
