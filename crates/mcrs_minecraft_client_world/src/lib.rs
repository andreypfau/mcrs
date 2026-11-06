//! The world state of a Minecraft client without a window or a renderer: the session's
//! registries, the received columns, the local player, the game mode and the chat.
// std: bevy

#[cfg(not(target_family = "wasm"))]
pub mod asset_source;
pub mod block_prediction;
pub mod block_target;
pub mod chat;
pub mod chat_history;
mod chat_packets;
pub mod columns;
pub mod game_mode;
pub mod game_packets;
pub mod joined_game;
pub mod local_player;
pub mod movement_input;
pub mod player;
pub mod player_view;
mod received;
pub mod registries;
pub mod screen;
