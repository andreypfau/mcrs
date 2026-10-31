// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const OVERWORLD: ResourceKey<'static, crate::DimensionType> = ResourceKey::new(rl!("minecraft:overworld"));
pub const OVERWORLD_CAVES: ResourceKey<'static, crate::DimensionType> = ResourceKey::new(rl!("minecraft:overworld_caves"));
pub const THE_END: ResourceKey<'static, crate::DimensionType> = ResourceKey::new(rl!("minecraft:the_end"));
pub const THE_NETHER: ResourceKey<'static, crate::DimensionType> = ResourceKey::new(rl!("minecraft:the_nether"));
