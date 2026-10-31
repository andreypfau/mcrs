// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const CUSTOM_OPTIONS: ResourceKey<'static, crate::dialog::Dialog> = ResourceKey::new(rl!("minecraft:custom_options"));
pub const QUICK_ACTIONS: ResourceKey<'static, crate::dialog::Dialog> = ResourceKey::new(rl!("minecraft:quick_actions"));
pub const SERVER_LINKS: ResourceKey<'static, crate::dialog::Dialog> = ResourceKey::new(rl!("minecraft:server_links"));
