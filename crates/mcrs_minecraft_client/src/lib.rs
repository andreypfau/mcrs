#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "an application has no caller to return an error to"
)]
#![recursion_limit = "256"]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use mcrs_minecraft_client_world::{block_breaking, local_player, registries};

#[cfg(target_os = "macos")]
pub mod app_nap;
pub mod camera;
pub mod cave;
mod client_terrain_plugin;
pub mod config;
#[cfg(feature = "dev")]
pub mod dev;
pub mod entities;
pub mod frozen_clocks;
#[cfg(not(target_family = "wasm"))]
pub mod game_directory;
mod glyph_page;
pub mod hand_keys_plugin;
pub mod input;
pub mod light_volume;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
pub mod local_server;
pub mod name_tag;
pub mod options;
#[cfg(not(target_family = "wasm"))]
mod options_file;
pub mod player_skins;
#[cfg(all(feature = "scene", not(target_family = "wasm")))]
pub mod scene_capture;
#[cfg(all(feature = "scene", not(target_family = "wasm")))]
pub mod scene_record;
#[cfg(not(target_family = "wasm"))]
pub mod screenshot;
#[cfg(not(target_family = "wasm"))]
pub mod settle;
pub mod sign_text;
#[cfg(not(target_family = "wasm"))]
mod skin_download;
pub mod sky;
pub mod sky_state;
pub mod stream;
pub mod terrain_debug_entry;
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub mod test_support;
pub mod tint;
#[cfg(not(target_family = "wasm"))]
mod title;
pub mod vanilla;
#[cfg(target_family = "wasm")]
pub mod web;
#[cfg(not(target_family = "wasm"))]
pub mod window_options;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
pub mod world_list;

pub use client_terrain_plugin::ClientTerrainPlugin;

#[derive(Message)]
pub struct Unsettled;

pub struct ClientPlugins(pub config::Knobs);

impl PluginGroup for ClientPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(vanilla::VanillaAssetsPlugin)
            .add(input::ClientInputPlugin)
            .add(hand_keys_plugin::HandKeysPlugin)
            .add(local_player::LocalPlayerPlugin)
            .add(mcrs_minecraft_client_world::game_mode::GameModePlugin)
            .add(block_breaking::BlockBreakingPlugin)
            .add(mcrs_minecraft_client_world::hotbar_sync::HotbarSyncPlugin)
            .add(options::ClientOptionsPlugin {
                options: self.0.options(),
                file: self.0.options_file(),
                gui_scale: self.0.gui_scale(),
            })
            .add(camera::CameraPlugin)
            .add(mcrs_minecraft_client_gui::debug_screen::DebugScreenPlugin {
                stats_interval: self.0.stats_interval(),
                always_time_passes: self.0.always_time_passes(),
            })
            .add(mcrs_minecraft_client_gui::chunk_map::ChunkMapPlugin {
                visible: self.0.chunk_map(),
            })
            .add(mcrs_minecraft_client_gui::light_levels::LightLevelsPlugin::<
                camera::PlayerCamera,
            >::new(self.0.light_levels()))
            .add(registries::ClientRegistriesPlugin::default())
            .add(sky::SkyPlugin)
            .add(frozen_clocks::FrozenClocksPlugin)
    }
}
