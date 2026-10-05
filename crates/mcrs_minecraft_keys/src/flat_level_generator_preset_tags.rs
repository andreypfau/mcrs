// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const VISIBLE: TagKey<crate::FlatLevelGeneratorPreset, &'static str> = TagKey::new(rl!("minecraft:visible"));
