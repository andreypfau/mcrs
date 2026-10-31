// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const COLD: ResourceKey<'static, crate::variant::CowVariant> = ResourceKey::new(rl!("minecraft:cold"));
pub const TEMPERATE: ResourceKey<'static, crate::variant::CowVariant> = ResourceKey::new(rl!("minecraft:temperate"));
pub const WARM: ResourceKey<'static, crate::variant::CowVariant> = ResourceKey::new(rl!("minecraft:warm"));
