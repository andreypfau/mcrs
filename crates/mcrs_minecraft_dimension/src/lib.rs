#[rustfmt::skip]
pub mod keys;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_registry::{HolderSet, Id};
use serde::{Deserialize, Serialize};

/// The part of a dimension type every crate that names one can hold. Its
/// environment attributes, timelines and default clock are a column beside the
/// registry.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: HolderSet<Block>,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntProvider,
    pub skybox: Skybox,
    pub cardinal_light: CardinalLight,
    pub has_fixed_time: Option<bool>,
}

/// The part of a dimension every crate that names one can hold. Its chunk
/// generator is a column beside the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimension {
    pub dimension_type: Id<crate::DimensionType>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skybox {
    #[default]
    #[serde(rename = "overworld")]
    Overworld,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "end")]
    End,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CardinalLight {
    #[default]
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "nether")]
    Nether,
}
