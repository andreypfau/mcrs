// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const CUSTOM_OPTIONS: ResourceKey<crate::dialog::Dialog, &'static str> = ResourceKey::new(rl!("minecraft:custom_options"));
pub const QUICK_ACTIONS: ResourceKey<crate::dialog::Dialog, &'static str> = ResourceKey::new(rl!("minecraft:quick_actions"));
pub const SERVER_LINKS: ResourceKey<crate::dialog::Dialog, &'static str> = ResourceKey::new(rl!("minecraft:server_links"));
