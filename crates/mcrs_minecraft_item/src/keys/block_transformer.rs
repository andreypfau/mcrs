// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const AXE: ResourceKey<'static, crate::block_transformer::BlockTransformer> = ResourceKey::new(rl!("minecraft:axe"));
pub const HOE: ResourceKey<'static, crate::block_transformer::BlockTransformer> = ResourceKey::new(rl!("minecraft:hoe"));
pub const SHOVEL: ResourceKey<'static, crate::block_transformer::BlockTransformer> = ResourceKey::new(rl!("minecraft:shovel"));
