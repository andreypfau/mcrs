// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const AMPLIFIED: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:amplified"));
pub const CAVES: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:caves"));
pub const END: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:end"));
pub const FLOATING_ISLANDS: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:floating_islands"));
pub const LARGE_BIOMES: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:large_biomes"));
pub const NETHER: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:nether"));
pub const OVERWORLD: ResourceKey<'static, crate::NoiseSettings> =
    ResourceKey::new(rl!("minecraft:overworld"));
