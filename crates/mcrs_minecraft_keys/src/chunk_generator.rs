// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const NOISE: StaticResourceLocation = rl!("minecraft:noise");
pub const FLAT: StaticResourceLocation = rl!("minecraft:flat");
pub const DEBUG: StaticResourceLocation = rl!("minecraft:debug");

pub const ENTRIES: &[StaticResourceLocation] = &[
    NOISE,
    FLAT,
    DEBUG,
];
