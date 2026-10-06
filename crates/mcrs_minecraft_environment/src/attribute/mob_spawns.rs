use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::NonNegativeInt;
use mcrs_minecraft_entity::spawn::{MobCategory, SpawnerData};
use mcrs_minecraft_value_provider::IntProvider;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// The `minecraft:gameplay/natural_mob_spawns` attribute argument.
///
/// A category absent from `spawns_by_category` is undefined and falls through
/// to the layer below; a category present but empty suppresses it, which is how
/// `deep_dark` and `the_void` silence the dimension's spawns.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MobSpawnSettings {
    pub spawn_costs: BTreeMap<ResourceLocation<Arc<str>>, SpawnCost>,
    pub spawns_by_category: BTreeMap<MobCategory, Vec<SpawnerData>>,
}

const FIELDS: &[&str] = &["spawn_costs", "spawns_by_category"];

impl<'de> Deserialize<'de> for MobSpawnSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SettingsVisitor;

        impl<'de> Visitor<'de> for SettingsVisitor {
            type Value = MobSpawnSettings;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("mob spawn settings")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<MobSpawnSettings, A::Error> {
                let mut spawn_costs = None;
                let mut spawns_by_category = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "spawn_costs" if spawn_costs.is_none() => {
                            spawn_costs = Some(map.next_value()?);
                        }
                        "spawns_by_category" if spawns_by_category.is_none() => {
                            spawns_by_category = Some(map.next_value()?);
                        }
                        "spawn_costs" => return Err(A::Error::duplicate_field("spawn_costs")),
                        "spawns_by_category" => {
                            return Err(A::Error::duplicate_field("spawns_by_category"));
                        }
                        other => return Err(A::Error::unknown_field(other, FIELDS)),
                    }
                }
                Ok(MobSpawnSettings {
                    spawn_costs: spawn_costs
                        .ok_or_else(|| A::Error::missing_field("spawn_costs"))?,
                    spawns_by_category: spawns_by_category
                        .ok_or_else(|| A::Error::missing_field("spawns_by_category"))?,
                })
            }
        }

        deserializer.deserialize_map(SettingsVisitor)
    }
}

impl MobSpawnSettings {
    /// Every category defined and empty, which silences the spawns of the
    /// layers below instead of falling through to them.
    pub fn no_spawns() -> Self {
        MobSpawnSettings {
            spawn_costs: BTreeMap::new(),
            spawns_by_category: MobCategory::ALL
                .iter()
                .map(|category| (*category, Vec::new()))
                .collect(),
        }
    }

    pub fn add_spawn(
        &mut self,
        category: MobCategory,
        entity: impl Into<ResourceLocation>,
        weight: NonNegativeInt,
        count: IntProvider,
    ) {
        self.spawns_by_category
            .entry(category)
            .or_default()
            .push(SpawnerData {
                entity: entity.into(),
                count,
                weight,
            });
    }

    pub fn add_cost(
        &mut self,
        entity: impl Into<ResourceLocation>,
        charge: f64,
        energy_budget: f64,
    ) {
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
