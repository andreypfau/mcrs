#![recursion_limit = "2048"]
#![allow(
    dead_code,
    unexpected_cfgs,
    clippy::type_complexity,
    clippy::too_many_arguments
)]

extern crate core;

pub mod block_light_table;
mod client_info;
pub mod runner;
pub use runner::{DEFAULT_TPS, run_server_loop};
pub mod configuration;
pub mod disconnect;
mod keep_alive;
pub mod login;
mod tag;
mod version;
mod weight;
pub mod world;

use crate::block_light_table::BlockLightTablePlugin;
use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::LoginPlugin;
use crate::world::WorldPlugin;
use bevy_app::{App, Plugin};
use mcrs_minecraft_network::NetworkPlugin;
use mcrs_voxel_server::VoxelServerPlugin;

pub struct MinecraftServerPlugin;

impl Plugin for MinecraftServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
        });
        app.add_plugins(mcrs_minecraft_core::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin);
        app.add_plugins(NetworkPlugin);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(WorldPlugin);
        app.add_plugins(BlockLightTablePlugin);
        app.add_plugins(ClientInfoPlugin);
    }
}
