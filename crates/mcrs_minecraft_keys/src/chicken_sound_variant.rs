// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const CLASSIC: ResourceKey<crate::ChickenSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:classic"));
pub const PICKY: ResourceKey<crate::ChickenSoundVariant, &'static str> = ResourceKey::new(rl!("minecraft:picky"));
