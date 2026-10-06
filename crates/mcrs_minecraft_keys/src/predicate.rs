// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const BLOCK_FAST_COOKING: ResourceKey<crate::Predicate, &'static str> = ResourceKey::new(rl!("minecraft:block/fast_cooking"));
pub const TOOL_CAN_SHEAR: ResourceKey<crate::Predicate, &'static str> = ResourceKey::new(rl!("minecraft:tool/can_shear"));
pub const TOOL_CAN_SILK_TOUCH: ResourceKey<crate::Predicate, &'static str> = ResourceKey::new(rl!("minecraft:tool/can_silk_touch"));
