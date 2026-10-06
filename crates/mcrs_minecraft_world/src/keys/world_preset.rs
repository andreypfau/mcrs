// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const AMPLIFIED: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:amplified"));
pub const DEBUG_ALL_BLOCK_STATES: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:debug_all_block_states"));
pub const FLAT: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:flat"));
pub const FLAT_ALL_DIMENSIONS: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:flat_all_dimensions"));
pub const LARGE_BIOMES: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:large_biomes"));
pub const NORMAL: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:normal"));
pub const SINGLE_BIOME_SURFACE: ResourceKey<crate::worldgen::world_preset::WorldPreset, &'static str> = ResourceKey::new(rl!("minecraft:single_biome_surface"));
