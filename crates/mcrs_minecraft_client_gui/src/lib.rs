//! The GUI of a Minecraft client: the hotbar, the inventory and chat screens, the game mode
//! switcher, the item models drawn in slots, and the F3 debug screen with its overlays.
// std: bevy
// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

pub mod chat;
pub mod chat_input;
pub mod chat_suggestions;
pub mod chunk_map;
pub mod cursor_grab;
pub mod debug_displayer;
pub mod debug_entry_fps;
pub mod debug_entry_network;
pub mod debug_entry_position;
pub mod debug_entry_system_specs;
pub mod debug_screen;
pub mod debug_screen_overlay;
pub mod font;
pub mod game_mode_switcher;
pub mod gui_items;
pub mod hotbar;
pub mod inventory_screen;
pub mod item_decorations;
pub mod item_resolve;
pub mod language;
pub mod light_levels;
pub mod scene;
pub mod sprite;
pub mod text;
pub mod tooltip;
