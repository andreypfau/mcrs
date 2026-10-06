// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const CLASSIC: ResourceKey<crate::variant::CatSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:classic"));
pub const ROYAL: ResourceKey<crate::variant::CatSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:royal"));
