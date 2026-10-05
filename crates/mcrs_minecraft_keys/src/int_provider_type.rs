// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BIASED_TO_BOTTOM: Id<crate::IntProviderType> = Id::from_static(2);
pub const CLAMPED: Id<crate::IntProviderType> = Id::from_static(4);
pub const CLAMPED_NORMAL: Id<crate::IntProviderType> = Id::from_static(6);
pub const CONSTANT: Id<crate::IntProviderType> = Id::from_static(0);
pub const TRAPEZOID: Id<crate::IntProviderType> = Id::from_static(7);
pub const UNIFORM: Id<crate::IntProviderType> = Id::from_static(1);
pub const VERY_BIASED_TO_BOTTOM: Id<crate::IntProviderType> = Id::from_static(3);
pub const WEIGHTED_LIST: Id<crate::IntProviderType> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:constant",
    "minecraft:uniform",
    "minecraft:biased_to_bottom",
    "minecraft:very_biased_to_bottom",
    "minecraft:clamped",
    "minecraft:weighted_list",
    "minecraft:clamped_normal",
    "minecraft:trapezoid",
];
