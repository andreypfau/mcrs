//! The GUI of a Minecraft client: the hotbar, the inventory and chat screens, the game mode
//! switcher, the item models drawn in slots, and the F3 debug screen with its overlays.
// std: bevy
// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod accessibility_settings_screen;
mod atlas_quad;
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
mod edit_box_draw;
pub mod font;
pub mod game_mode_switcher;
pub mod gui_items;
pub mod gui_sprite_id;
pub mod gui_texture_id;
pub mod hotbar;
pub mod inventory_screen;
pub mod item_decorations;
pub mod item_resolve;
pub mod language;
pub mod light_levels;
pub mod menu;
mod menu_cursor;
pub mod menu_draw;
pub mod menu_event;
mod menu_input;
pub mod menu_input_systems;
pub mod menu_options_systems;
pub mod menu_plugin;
pub mod menu_screens_plugin;
mod option_caption;
mod option_sub_screen;
mod options_screen;
mod pause_screen;
pub mod scene;
mod scroll_area_draw;
pub mod sprite;
mod tab_bar_draw;
pub mod text;
mod texture_tile;
pub mod tooltip;
mod video_settings_screen;
pub mod vitals;
