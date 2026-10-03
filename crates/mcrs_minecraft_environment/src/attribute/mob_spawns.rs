use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::value_provider::IntProvider;
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
    #[serde(default)]
    pub spawn_costs: BTreeMap<ResourceLocation<Arc<str>>, SpawnCost>,
    #[serde(default)]
    pub spawns_by_category: BTreeMap<MobCategory, Vec<SpawnerData>>,
}

impl MobSpawnSettings {
    /// Every category defined and empty, which silences the spawns of the
    /// layers below instead of falling through to them.
    pub fn no_spawns() -> Self {
        MobSpawnSettings {
            spawn_costs: BTreeMap::new(),
            spawns_by_category: MobCategory::ALL.map(|category| (category, Vec::new())).into(),
        }
    }

    pub fn add_spawn(
        &mut self,
        category: MobCategory,
        entity: impl Into<ResourceLocation>,
        weight: i32,
        count: IntProvider,
    ) {
        self.spawns_by_category
            .entry(category)
            .or_default()
            .push(SpawnerData {
                entity: entity.into(),
                count,
                weight: Bounded(weight),
            });
    }

    pub fn add_cost(&mut self, entity: impl Into<ResourceLocation>, charge: f64, energy_budget: f64) {
        self.spawn_costs.insert(
            entity.into(),
            SpawnCost {
                charge,
                energy_budget,
            },
        );
    }

    pub fn is_empty(&self) -> bool {
        self.spawn_costs.is_empty() && self.spawns_by_category.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnCost {
    pub charge: f64,
    pub energy_budget: f64,
}
