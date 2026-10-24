use mcrs_minecraft_registries::{Refs, Resolved, dispatch, registry_value};
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::{Deserialize, Serialize};

/// An entry of `worldgen/structure`. A crate of world generation holds its value, so a spawn
/// condition names its structures without that crate.
pub enum Structure {}

/// One `spawn_conditions` entry of a variant asset: a priority, and a
/// condition that an absent field leaves always true.
#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnSelector<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<SpawnCondition<M>>,
    pub priority: i32,
}

#[registry_value]
#[dispatch(registry = crate::keys::SpawnConditionType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum SpawnCondition<M: Refs = Resolved> {
    Structure {
        structures: M::Set<Structure>,
    },
    Biome {
        biomes: M::Set<mcrs_minecraft_biome::Biome>,
    },
    MoonBrightness {
        range: MinMaxBounds<f64>,
    },
}
