// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CONTEXT: Id<crate::LootNbtProviderType> = Id::from_static(1);
pub const STORAGE: Id<crate::LootNbtProviderType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:storage",
    "minecraft:context",
];
