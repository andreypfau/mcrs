// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const NETHER: ResourceKey<crate::parameter_list::MultiNoiseBiomeSourceParameterList, &'static str> = ResourceKey::new(rl!("minecraft:nether"));
pub const OVERWORLD: ResourceKey<crate::parameter_list::MultiNoiseBiomeSourceParameterList, &'static str> = ResourceKey::new(rl!("minecraft:overworld"));
