// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const BIG: ResourceKey<'static, crate::variant::PigSoundVariant> = ResourceKey::new(rl!("minecraft:big"));
pub const CLASSIC: ResourceKey<'static, crate::variant::PigSoundVariant> = ResourceKey::new(rl!("minecraft:classic"));
pub const MINI: ResourceKey<'static, crate::variant::PigSoundVariant> = ResourceKey::new(rl!("minecraft:mini"));
