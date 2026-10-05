// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const IN_END: TagKey<crate::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_end"));
pub const IN_NETHER: TagKey<crate::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_nether"));
pub const IN_OVERWORLD: TagKey<crate::Timeline, &'static str> = TagKey::new(rl!("minecraft:in_overworld"));
pub const UNIVERSAL: TagKey<crate::Timeline, &'static str> = TagKey::new(rl!("minecraft:universal"));
