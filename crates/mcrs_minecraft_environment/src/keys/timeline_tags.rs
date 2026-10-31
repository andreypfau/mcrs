// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{TagKey, rl};

pub const IN_END: TagKey<'static, crate::timeline::Timeline> = TagKey::new(rl!("minecraft:in_end"));
pub const IN_NETHER: TagKey<'static, crate::timeline::Timeline> = TagKey::new(rl!("minecraft:in_nether"));
pub const IN_OVERWORLD: TagKey<'static, crate::timeline::Timeline> = TagKey::new(rl!("minecraft:in_overworld"));
pub const UNIVERSAL: TagKey<'static, crate::timeline::Timeline> = TagKey::new(rl!("minecraft:universal"));
