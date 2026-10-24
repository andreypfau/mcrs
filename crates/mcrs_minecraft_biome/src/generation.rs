use serde::{Deserialize, Serialize};

/// An entry of `worldgen/carver`. A crate of world generation holds its value, so a biome names
/// its carvers without that crate.
pub enum ConfiguredCarver {}

/// An entry of `worldgen/placed_feature`. A crate of world generation holds its value, so a biome
/// names its features without that crate.
pub enum PlacedFeature {}

// Declaration order is the step index; keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    FluidSprings,
    VegetalDecoration,
    TopLayerModification,
}
