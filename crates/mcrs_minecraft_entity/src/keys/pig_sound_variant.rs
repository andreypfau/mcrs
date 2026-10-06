// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const BIG: ResourceKey<crate::variant::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:big"));
pub const CLASSIC: ResourceKey<crate::variant::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:classic"));
pub const MINI: ResourceKey<crate::variant::PigSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:mini"));
