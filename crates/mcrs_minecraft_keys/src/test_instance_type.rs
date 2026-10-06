// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BLOCK_BASED: StaticResourceLocation = rl!("minecraft:block_based");
pub const FUNCTION: StaticResourceLocation = rl!("minecraft:function");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BLOCK_BASED,
    FUNCTION,
];
