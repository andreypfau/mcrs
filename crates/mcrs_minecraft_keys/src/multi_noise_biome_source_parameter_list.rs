// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const NETHER: ResourceKey<crate::MultiNoiseBiomeSourceParameterList, &'static str> = ResourceKey::new(rl!("minecraft:nether"));
pub const OVERWORLD: ResourceKey<crate::MultiNoiseBiomeSourceParameterList, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
