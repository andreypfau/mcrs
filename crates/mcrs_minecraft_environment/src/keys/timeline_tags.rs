// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const IN_END: TagKey<crate::timeline::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_end"));
pub const IN_NETHER: TagKey<crate::timeline::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_nether"));
pub const IN_OVERWORLD: TagKey<crate::timeline::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_overworld"));
pub const UNIVERSAL: TagKey<crate::timeline::Timeline, &'static str> = TagKey::new(rl!("minecraft:universal"));
