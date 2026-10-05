// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const BIG: ResourceKey<crate::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:big"));
pub const CLASSIC: ResourceKey<crate::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:classic"));
pub const MINI: ResourceKey<crate::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:mini"));
