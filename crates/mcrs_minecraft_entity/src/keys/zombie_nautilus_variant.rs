// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const TEMPERATE: ResourceKey<crate::variant::ZombieNautilusVariant, &'static str> = ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<crate::variant::ZombieNautilusVariant, &'static str> = ResourceKey::new(rl!("minecraft:warm"));
