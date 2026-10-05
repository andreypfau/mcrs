// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const ALWAYS_PASS: ResourceKey<crate::TestInstance, &'static str> = ResourceKey::new(rl!("minecraft:always_pass"));
