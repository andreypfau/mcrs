// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const CAN_SPAWN_FROM_BONE_MEAL: TagKey<crate::proto::Feature, &'static str> = TagKey::new(rl!("minecraft:can_spawn_from_bone_meal"));
