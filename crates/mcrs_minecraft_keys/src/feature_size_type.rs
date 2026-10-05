// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const THREE_LAYERS_FEATURE_SIZE: Id<crate::FeatureSizeType> = Id::from_static(1);
pub const TWO_LAYERS_FEATURE_SIZE: Id<crate::FeatureSizeType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:two_layers_feature_size",
    "minecraft:three_layers_feature_size",
];
