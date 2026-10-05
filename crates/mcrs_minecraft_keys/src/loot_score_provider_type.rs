// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CONTEXT: Id<crate::LootScoreProviderType> = Id::from_static(1);
pub const FIXED: Id<crate::LootScoreProviderType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:fixed",
    "minecraft:context",
];
