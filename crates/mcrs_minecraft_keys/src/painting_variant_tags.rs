// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const PLACEABLE: TagKey<crate::PaintingVariant, &'static str> = TagKey::new(rl!("minecraft:placeable"));
