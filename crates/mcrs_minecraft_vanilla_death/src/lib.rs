#![no_std]

extern crate alloc;

mod announce_deaths;
mod death_drops_plugin;
mod death_message_text;
mod death_messages_plugin;
mod describe_player_deaths;
mod drop_held_stacks;
mod drop_inventory;

pub use death_drops_plugin::DeathDropsPlugin;
pub use death_message_text::death_message_text;
pub use death_messages_plugin::DeathMessagesPlugin;
