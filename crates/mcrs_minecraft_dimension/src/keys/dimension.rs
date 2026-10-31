// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const OVERWORLD: ResourceKey<'static, crate::Dimension> = ResourceKey::new(rl!("minecraft:overworld"));
pub const THE_END: ResourceKey<'static, crate::Dimension> = ResourceKey::new(rl!("minecraft:the_end"));
pub const THE_NETHER: ResourceKey<'static, crate::Dimension> = ResourceKey::new(rl!("minecraft:the_nether"));
