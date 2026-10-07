use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::component::predicate::{BlockPredicate, StatePropertiesPredicate};
use mcrs_minecraft_registry::HolderSet;
use mcrs_minecraft_worldgen_structure::Structure;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biomes: Option<HolderSet<Biome>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structures: Option<HolderSet<Structure>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<ResourceKey<Dimension>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub smokey: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<BlockPredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid: Option<FluidPredicate>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "mcrs_minecraft_core::codec::optional_flag"
    )]
    pub can_see_sky: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub x: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub y: MinMaxBounds<f64>,
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub z: MinMaxBounds<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightPredicate {
    #[serde(default, skip_serializing_if = "MinMaxBounds::is_any")]
    pub light: MinMaxBounds<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FluidPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluids: Option<HolderSet<Fluid>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StatePropertiesPredicate>,
}
