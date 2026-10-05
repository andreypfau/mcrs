// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const OVERWORLD: ResourceKey<crate::DimensionType, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
pub const OVERWORLD_CAVES: ResourceKey<crate::DimensionType, &'static str> = ResourceKey::new(rl!("minecraft:overworld_caves"));
pub const THE_END: ResourceKey<crate::DimensionType, &'static str> = ResourceKey::new(rl!("minecraft:the_end"));
pub const THE_NETHER: ResourceKey<crate::DimensionType, &'static str> = ResourceKey::new(rl!("minecraft:the_nether"));
