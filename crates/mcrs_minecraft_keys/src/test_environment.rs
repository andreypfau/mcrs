// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const DEFAULT: ResourceKey<crate::TestEnvironment, &'static str> = ResourceKey::new(rl!("minecraft:default"));
