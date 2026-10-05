// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BIOME: Id<crate::SpawnConditionType> = Id::from_static(2);
pub const MOON_BRIGHTNESS: Id<crate::SpawnConditionType> = Id::from_static(1);
pub const STRUCTURE: Id<crate::SpawnConditionType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:structure",
    "minecraft:moon_brightness",
    "minecraft:biome",
];
