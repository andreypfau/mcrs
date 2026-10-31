#![recursion_limit = "2048"]
// std: bevy_ecs

pub mod configuration;
mod data_pack;
mod dimension_worlds_plugin;
mod keep_alive;
pub mod login;
mod require;
pub mod runner;
mod server_core_plugin;
mod server_network_plugin;
mod status;
pub mod world_options;

pub use data_pack::{DataPack, DataPackError};
pub use dimension_worlds_plugin::DimensionWorldsPlugin;
pub use login::NetworkCompressionThreshold;
#[cfg(feature = "client-jar")]
pub use mcrs_minecraft_client_jar::ClientJar;
pub use mcrs_minecraft_game_mode::GameMode;
pub use mcrs_minecraft_level::server_loop::spawn_server_thread;
pub use mcrs_minecraft_network::BoundAddress;
pub use runner::{DEFAULT_TPS, run_server_loop};
pub use server_core_plugin::ServerCorePlugin;
pub use server_network_plugin::ServerNetworkPlugin;
