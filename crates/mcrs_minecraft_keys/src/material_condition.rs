// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const DEEP_UNDER_FLOOR: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:deep_under_floor"));
pub const NOT_UNDER_DEEP_WATER: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:not_under_deep_water"));
pub const NOT_UNDERWATER: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:not_underwater"));
pub const ON_CEILING: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:on_ceiling"));
pub const ON_FLOOR: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:on_floor"));
pub const UNDER_CEILING: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:under_ceiling"));
pub const UNDER_FLOOR: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:under_floor"));
pub const VERY_DEEP_UNDER_FLOOR: ResourceKey<crate::MaterialCondition, &'static str> = ResourceKey::new(rl!("minecraft:very_deep_under_floor"));
