// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{TagKey, rl};

pub const PAUSE_SCREEN_ADDITIONS: TagKey<crate::Dialog, &'static str> = TagKey::new(rl!("minecraft:pause_screen_additions"));
pub const QUICK_ACTIONS: TagKey<crate::Dialog, &'static str> = TagKey::new(rl!("minecraft:quick_actions"));
