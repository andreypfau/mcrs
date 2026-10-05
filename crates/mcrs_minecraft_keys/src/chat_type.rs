// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_core::{ResourceKey, rl};

pub const CHAT: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:chat"));
pub const EMOTE_COMMAND: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:emote_command"));
pub const MSG_COMMAND_INCOMING: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:msg_command_incoming"));
pub const MSG_COMMAND_OUTGOING: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:msg_command_outgoing"));
pub const SAY_COMMAND: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:say_command"));
pub const TEAM_MSG_COMMAND_INCOMING: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:team_msg_command_incoming"));
pub const TEAM_MSG_COMMAND_OUTGOING: ResourceKey<crate::ChatType, &'static str> = ResourceKey::new(rl!("minecraft:team_msg_command_outgoing"));
