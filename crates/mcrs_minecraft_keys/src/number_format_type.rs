// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const BLANK: StaticResourceLocation = rl!("minecraft:blank");
pub const STYLED: StaticResourceLocation = rl!("minecraft:styled");
pub const FIXED: StaticResourceLocation = rl!("minecraft:fixed");

pub const ENTRIES: &[StaticResourceLocation] = &[
    BLANK,
    STYLED,
    FIXED,
];
