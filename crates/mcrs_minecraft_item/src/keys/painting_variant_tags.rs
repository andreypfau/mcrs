// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{TagKey, rl};

pub const PLACEABLE: TagKey<'static, crate::PaintingVariantValue> = TagKey::new(rl!("minecraft:placeable"));
