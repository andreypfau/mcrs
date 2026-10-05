// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const TEMPERATE: ResourceKey<crate::ZombieNautilusVariant, &'static str> = ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<crate::ZombieNautilusVariant, &'static str> = ResourceKey::new(rl!("minecraft:warm"));
