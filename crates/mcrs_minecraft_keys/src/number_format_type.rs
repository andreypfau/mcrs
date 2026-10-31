// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const BLANK: ResourceLocation<'static> = rl!("minecraft:blank");
pub const STYLED: ResourceLocation<'static> = rl!("minecraft:styled");
pub const FIXED: ResourceLocation<'static> = rl!("minecraft:fixed");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    BLANK,
    STYLED,
    FIXED,
];
