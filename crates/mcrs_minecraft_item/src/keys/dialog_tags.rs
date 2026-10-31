// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{TagKey, rl};

pub const PAUSE_SCREEN_ADDITIONS: TagKey<'static, crate::dialog::Dialog> = TagKey::new(rl!("minecraft:pause_screen_additions"));
pub const QUICK_ACTIONS: TagKey<'static, crate::dialog::Dialog> = TagKey::new(rl!("minecraft:quick_actions"));
