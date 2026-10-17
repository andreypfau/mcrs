// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const SOUNDS_WOODEN: TagKey<crate::BlockSoundSet, &'static str> =
    TagKey::new(rl!("minecraft:sounds_wooden"));
