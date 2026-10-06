// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const EXTENDED: TagKey<crate::WorldPreset, &'static str> = TagKey::new(rl!("minecraft:extended"));
pub const NORMAL: TagKey<crate::WorldPreset, &'static str> = TagKey::new(rl!("minecraft:normal"));
