// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const BOTTOMLESS_PIT: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:bottomless_pit"));
pub const CLASSIC_FLAT: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:classic_flat"));
pub const DESERT: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:desert"));
pub const OVERWORLD: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
pub const REDSTONE_READY: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:redstone_ready"));
pub const SNOWY_KINGDOM: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:snowy_kingdom"));
pub const THE_VOID: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:the_void"));
pub const TUNNELERS_DREAM: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:tunnelers_dream"));
pub const WATER_WORLD: ResourceKey<crate::FlatLevelGeneratorPreset, &'static str> = ResourceKey::new(rl!("minecraft:water_world"));
