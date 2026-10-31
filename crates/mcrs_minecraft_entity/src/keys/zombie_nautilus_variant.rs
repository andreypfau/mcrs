// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const TEMPERATE: ResourceKey<'static, crate::variant::ZombieNautilusVariant> = ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<'static, crate::variant::ZombieNautilusVariant> = ResourceKey::new(rl!("minecraft:warm"));
