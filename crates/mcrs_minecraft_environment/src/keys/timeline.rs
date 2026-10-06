// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const DAY: ResourceKey<crate::timeline::Timeline, &'static str> = ResourceKey::new(rl!("minecraft:day"));
pub const EARLY_GAME: ResourceKey<crate::timeline::Timeline, &'static str> = ResourceKey::new(rl!("minecraft:early_game"));
pub const MOON: ResourceKey<crate::timeline::Timeline, &'static str> = ResourceKey::new(rl!("minecraft:moon"));
pub const VILLAGER_SCHEDULE: ResourceKey<crate::timeline::Timeline, &'static str> = ResourceKey::new(rl!("minecraft:villager_schedule"));
