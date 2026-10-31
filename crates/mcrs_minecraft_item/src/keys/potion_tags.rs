// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{TagKey, rl};

pub const DOUSES_FIRE: TagKey<'static, crate::keys::Potion> = TagKey::new(rl!("minecraft:douses_fire"));
pub const EXTINGUISHES_ENTITIES: TagKey<'static, crate::keys::Potion> = TagKey::new(rl!("minecraft:extinguishes_entities"));
pub const HURTS_WATER_SENSITIVE_ENTITIES: TagKey<'static, crate::keys::Potion> = TagKey::new(rl!("minecraft:hurts_water_sensitive_entities"));
pub const REHYDRATES_AXOLOTLS: TagKey<'static, crate::keys::Potion> = TagKey::new(rl!("minecraft:rehydrates_axolotls"));
pub const TRADEABLE: TagKey<'static, crate::keys::Potion> = TagKey::new(rl!("minecraft:tradeable"));
