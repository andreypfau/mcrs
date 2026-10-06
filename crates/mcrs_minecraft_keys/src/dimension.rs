// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const OVERWORLD: ResourceKey<crate::Dimension, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
pub const THE_END: ResourceKey<crate::Dimension, &'static str> = ResourceKey::new(rl!("minecraft:the_end"));
pub const THE_NETHER: ResourceKey<crate::Dimension, &'static str> = ResourceKey::new(rl!("minecraft:the_nether"));
