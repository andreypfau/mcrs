// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CLAMPED: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(0);
pub const EXPONENT: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(4);
pub const FRACTION: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(1);
pub const LEVELS_SQUARED: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(2);
pub const LINEAR: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(3);
pub const LOOKUP: Id<crate::EnchantmentLevelBasedValueType> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:clamped",
    "minecraft:fraction",
    "minecraft:levels_squared",
    "minecraft:linear",
    "minecraft:exponent",
    "minecraft:lookup",
];
