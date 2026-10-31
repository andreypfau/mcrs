// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const BLOCK_FAST_COOKING: ResourceKey<'static, crate::condition::LootCondition> = ResourceKey::new(rl!("minecraft:block/fast_cooking"));
pub const TOOL_CAN_SHEAR: ResourceKey<'static, crate::condition::LootCondition> = ResourceKey::new(rl!("minecraft:tool/can_shear"));
pub const TOOL_CAN_SILK_TOUCH: ResourceKey<'static, crate::condition::LootCondition> = ResourceKey::new(rl!("minecraft:tool/can_silk_touch"));
