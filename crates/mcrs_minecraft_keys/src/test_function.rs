// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{StaticResourceLocation, rl};

pub const ALWAYS_PASS: StaticResourceLocation = rl!("minecraft:always_pass");

pub const ENTRIES: &[StaticResourceLocation] = &[
    ALWAYS_PASS,
];
