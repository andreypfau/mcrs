// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const BIOMES: Id<crate::ChunkStatus> = Id::from_static(4);
pub const EMPTY: Id<crate::ChunkStatus> = Id::from_static(0);
pub const FEATURES: Id<crate::ChunkStatus> = Id::from_static(6);
pub const FULL: Id<crate::ChunkStatus> = Id::from_static(10);
pub const INITIALIZE_LIGHT: Id<crate::ChunkStatus> = Id::from_static(7);
pub const LIGHT: Id<crate::ChunkStatus> = Id::from_static(8);
pub const NOISE_BIOMES: Id<crate::ChunkStatus> = Id::from_static(3);
pub const SPAWN: Id<crate::ChunkStatus> = Id::from_static(9);
pub const STRUCTURE_REFERENCES: Id<crate::ChunkStatus> = Id::from_static(2);
pub const STRUCTURE_STARTS: Id<crate::ChunkStatus> = Id::from_static(1);
pub const TERRAIN: Id<crate::ChunkStatus> = Id::from_static(5);

pub const NAMES: &[&str] = &[
    "minecraft:empty",
    "minecraft:structure_starts",
    "minecraft:structure_references",
    "minecraft:noise_biomes",
    "minecraft:biomes",
    "minecraft:terrain",
    "minecraft:features",
    "minecraft:initialize_light",
    "minecraft:light",
    "minecraft:spawn",
    "minecraft:full",
];
