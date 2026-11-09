// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const OVERWORLD: ResourceKey<'static, crate::WorldClock> =
    ResourceKey::new(rl!("minecraft:overworld"));
pub const THE_END: ResourceKey<'static, crate::WorldClock> =
    ResourceKey::new(rl!("minecraft:the_end"));
