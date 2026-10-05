// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const ALLAY_CAN_LISTEN: TagKey<crate::GameEvent, &'static str> = TagKey::new(rl!("minecraft:allay_can_listen"));
pub const IGNORE_VIBRATIONS_SNEAKING: TagKey<crate::GameEvent, &'static str> = TagKey::new(rl!("minecraft:ignore_vibrations_sneaking"));
pub const SHRIEKER_CAN_LISTEN: TagKey<crate::GameEvent, &'static str> = TagKey::new(rl!("minecraft:shrieker_can_listen"));
pub const VIBRATIONS: TagKey<crate::GameEvent, &'static str> = TagKey::new(rl!("minecraft:vibrations"));
pub const WARDEN_CAN_LISTEN: TagKey<crate::GameEvent, &'static str> = TagKey::new(rl!("minecraft:warden_can_listen"));
