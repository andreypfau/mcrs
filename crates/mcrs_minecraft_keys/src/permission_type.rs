// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const ATOM: StaticResourceLocation = rl!("minecraft:atom");
pub const COMMAND_LEVEL: StaticResourceLocation = rl!("minecraft:command_level");

pub const ENTRIES: &[StaticResourceLocation] = &[
    ATOM,
    COMMAND_LEVEL,
];
