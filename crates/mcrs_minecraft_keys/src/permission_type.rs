// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const ATOM: ResourceLocation<'static> = rl!("minecraft:atom");
pub const COMMAND_LEVEL: ResourceLocation<'static> = rl!("minecraft:command_level");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    ATOM,
    COMMAND_LEVEL,
];
