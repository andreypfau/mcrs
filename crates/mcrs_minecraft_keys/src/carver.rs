// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const CANYON: ResourceKey<crate::Carver, &'static str> = ResourceKey::new(rl!("minecraft:canyon"));
pub const CAVE: ResourceKey<crate::Carver, &'static str> = ResourceKey::new(rl!("minecraft:cave"));
pub const CAVE_EXTRA_UNDERGROUND: ResourceKey<crate::Carver, &'static str> = ResourceKey::new(rl!("minecraft:cave_extra_underground"));
pub const NETHER_CAVE: ResourceKey<crate::Carver, &'static str> = ResourceKey::new(rl!("minecraft:nether_cave"));
