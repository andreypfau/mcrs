// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BIASED_TO_BOTTOM: Id<crate::HeightProviderType> = Id::from_static(2);
pub const CONSTANT: Id<crate::HeightProviderType> = Id::from_static(0);
pub const TRAPEZOID: Id<crate::HeightProviderType> = Id::from_static(4);
pub const UNIFORM: Id<crate::HeightProviderType> = Id::from_static(1);
pub const VERY_BIASED_TO_BOTTOM: Id<crate::HeightProviderType> = Id::from_static(3);
pub const WEIGHTED_LIST: Id<crate::HeightProviderType> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:constant",
    "minecraft:uniform",
    "minecraft:biased_to_bottom",
    "minecraft:very_biased_to_bottom",
    "minecraft:trapezoid",
    "minecraft:weighted_list",
];
