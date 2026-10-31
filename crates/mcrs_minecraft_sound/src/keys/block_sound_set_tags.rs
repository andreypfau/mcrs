// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{TagKey, rl};

pub const SOUNDS_WOODEN: TagKey<'static, crate::BlockSoundSet> =
    TagKey::new(rl!("minecraft:sounds_wooden"));
