// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const NETHER: ResourceKey<'static, crate::parameter_list::MultiNoiseBiomeSourceParameterList> = ResourceKey::new(rl!("minecraft:nether"));
pub const OVERWORLD: ResourceKey<'static, crate::parameter_list::MultiNoiseBiomeSourceParameterList> = ResourceKey::new(rl!("minecraft:overworld"));
