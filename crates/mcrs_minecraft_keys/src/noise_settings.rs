// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const AMPLIFIED: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:amplified"));
pub const CAVES: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:caves"));
pub const END: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:end"));
pub const FLOATING_ISLANDS: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:floating_islands"));
pub const LARGE_BIOMES: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:large_biomes"));
pub const NETHER: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:nether"));
pub const OVERWORLD: ResourceKey<crate::NoiseSettings, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
