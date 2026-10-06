// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const AXE: ResourceKey<crate::block_transformer::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:axe"));
pub const HOE: ResourceKey<crate::block_transformer::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:hoe"));
pub const SHOVEL: ResourceKey<crate::block_transformer::BlockTransformer, &'static str> = ResourceKey::new(rl!("minecraft:shovel"));
