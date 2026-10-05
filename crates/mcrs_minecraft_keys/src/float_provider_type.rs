// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CLAMPED_NORMAL: Id<crate::FloatProviderType> = Id::from_static(2);
pub const CONSTANT: Id<crate::FloatProviderType> = Id::from_static(0);
pub const TRAPEZOID: Id<crate::FloatProviderType> = Id::from_static(3);
pub const UNIFORM: Id<crate::FloatProviderType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:constant",
    "minecraft:uniform",
    "minecraft:clamped_normal",
    "minecraft:trapezoid",
];
