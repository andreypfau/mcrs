// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const COLD: ResourceKey<crate::variant::CowVariant, &'static str> = ResourceKey::new(rl!("minecraft:cold"));
pub const TEMPERATE: ResourceKey<crate::variant::CowVariant, &'static str> = ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<crate::variant::CowVariant, &'static str> = ResourceKey::new(rl!("minecraft:warm"));
