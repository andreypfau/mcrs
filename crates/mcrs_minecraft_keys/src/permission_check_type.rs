// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const ALWAYS_PASS: ResourceLocation<'static> = rl!("minecraft:always_pass");
pub const REQUIRE: ResourceLocation<'static> = rl!("minecraft:require");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    ALWAYS_PASS,
    REQUIRE,
];
