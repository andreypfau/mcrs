// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ADD: Id<crate::EnchantmentValueEffectType> = Id::from_static(0);
pub const ALL_OF: Id<crate::EnchantmentValueEffectType> = Id::from_static(1);
pub const EXPONENTIAL: Id<crate::EnchantmentValueEffectType> = Id::from_static(4);
pub const MULTIPLY: Id<crate::EnchantmentValueEffectType> = Id::from_static(2);
pub const REMOVE_BINOMIAL: Id<crate::EnchantmentValueEffectType> = Id::from_static(3);
pub const SET: Id<crate::EnchantmentValueEffectType> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:add",
    "minecraft:all_of",
    "minecraft:multiply",
    "minecraft:remove_binomial",
    "minecraft:exponential",
    "minecraft:set",
];
