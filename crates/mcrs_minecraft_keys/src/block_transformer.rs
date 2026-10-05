// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const AXE: ResourceKey<crate::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:axe"));
pub const HOE: ResourceKey<crate::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:hoe"));
pub const SHOVEL: ResourceKey<crate::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:shovel"));
