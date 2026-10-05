// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BY_COST: Id<crate::EnchantmentProviderType> = Id::from_static(0);
pub const BY_COST_WITH_DIFFICULTY: Id<crate::EnchantmentProviderType> = Id::from_static(1);
pub const SINGLE: Id<crate::EnchantmentProviderType> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:by_cost",
    "minecraft:by_cost_with_difficulty",
    "minecraft:single",
];
