// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CHECKERBOARD: Id<crate::BiomeSource> = Id::from_static(2);
pub const FIXED: Id<crate::BiomeSource> = Id::from_static(0);
pub const MULTI_NOISE: Id<crate::BiomeSource> = Id::from_static(1);
pub const THE_END: Id<crate::BiomeSource> = Id::from_static(3);

pub const NAMES: &[&str] = &[
    "minecraft:fixed",
    "minecraft:multi_noise",
    "minecraft:checkerboard",
    "minecraft:the_end",
];
