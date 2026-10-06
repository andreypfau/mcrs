// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const BREWING_SPEED_DEFAULT: ResourceKey<crate::loot::ContextFloatProvider, &'static str> = ResourceKey::new(rl!("minecraft:brewing/speed_default"));
pub const COOKING_FAST_SPEED_MULTIPLIER: ResourceKey<crate::loot::ContextFloatProvider, &'static str> = ResourceKey::new(rl!("minecraft:cooking/fast_speed_multiplier"));
pub const COOKING_NORMAL_SPEED_MULTIPLIER: ResourceKey<crate::loot::ContextFloatProvider, &'static str> = ResourceKey::new(rl!("minecraft:cooking/normal_speed_multiplier"));
pub const COOKING_SPEED_DEFAULT: ResourceKey<crate::loot::ContextFloatProvider, &'static str> = ResourceKey::new(rl!("minecraft:cooking/speed_default"));
