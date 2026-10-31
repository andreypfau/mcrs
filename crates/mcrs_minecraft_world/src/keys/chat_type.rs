// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceKey, rl};

pub const CHAT: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:chat"));
pub const EMOTE_COMMAND: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:emote_command"));
pub const MSG_COMMAND_INCOMING: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:msg_command_incoming"));
pub const MSG_COMMAND_OUTGOING: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:msg_command_outgoing"));
pub const SAY_COMMAND: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:say_command"));
pub const TEAM_MSG_COMMAND_INCOMING: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:team_msg_command_incoming"));
pub const TEAM_MSG_COMMAND_OUTGOING: ResourceKey<'static, crate::chat_type::ChatType> = ResourceKey::new(rl!("minecraft:team_msg_command_outgoing"));
