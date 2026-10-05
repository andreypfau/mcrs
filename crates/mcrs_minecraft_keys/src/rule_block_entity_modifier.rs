// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const APPEND_LOOT: Id<crate::RuleBlockEntityModifier> = Id::from_static(3);
pub const APPEND_STATIC: Id<crate::RuleBlockEntityModifier> = Id::from_static(2);
pub const CLEAR: Id<crate::RuleBlockEntityModifier> = Id::from_static(0);
pub const PASSTHROUGH: Id<crate::RuleBlockEntityModifier> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:clear",
    "minecraft:passthrough",
    "minecraft:append_static",
    "minecraft:append_loot",
];
