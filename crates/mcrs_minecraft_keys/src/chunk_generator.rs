// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const DEBUG: Id<crate::ChunkGenerator> = Id::from_static(2);
pub const FLAT: Id<crate::ChunkGenerator> = Id::from_static(1);
pub const NOISE: Id<crate::ChunkGenerator> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:noise",
    "minecraft:flat",
    "minecraft:debug",
];
