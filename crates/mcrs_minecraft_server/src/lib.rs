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

use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::LoginPlugin;
use crate::world::WorldPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_network::NetworkPlugin;
use mcrs_voxel_server::VoxelServerPlugin;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::LazyLock;

pub use mcrs_minecraft_network::BoundAddress;
pub use mcrs_voxel_server::spawn_server_thread;

pub struct MinecraftServerPlugin {
    /// Port 0 asks the OS for a free port; read the result back from
    /// [`BoundAddress`].
    pub bind_address: SocketAddr,
    /// Clear when the server shares a process with another Bevy app, which
    /// then owns the process-global task pools.
    pub owns_task_pools: bool,
    /// Absolute path to the asset corpus, for a server that cannot resolve
    /// `assets` from its own working directory or executable location.
    pub asset_path: Option<String>,
    /// World folder to read saved chunks from. Without one, and for any column
    /// the folder has never saved, the dimension generates its terrain.
    pub world: Option<PathBuf>,
}

/// `MCRS_NO_LIGHTING=1` leaves the block light table unbuilt, so no dimension
/// registers a lighting engine and every column goes to the client at full sky
/// light. Sunlight, torches and shadows all stop existing; what is left is a
/// world that loads without the propagation cost.
pub fn lighting_disabled() -> bool {
    static DISABLED: LazyLock<bool> = LazyLock::new(|| {
        matches!(
            std::env::var("MCRS_NO_LIGHTING").as_deref(),
            Ok("1" | "true" | "on" | "yes")
        )
    });
    *DISABLED
}

/// The world folder the server reads its saved chunks from.
#[derive(Resource, Clone)]
pub struct WorldSave(pub PathBuf);

impl Default for MinecraftServerPlugin {
    fn default() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 25565).into(),
            owns_task_pools: true,
            asset_path: None,
            world: None,
        }
    }
}

impl MinecraftServerPlugin {
    /// A server running inside another process: loopback only, an OS-assigned
    /// port, and the task pools left to the host app.
    pub fn embedded() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
            owns_task_pools: false,
            asset_path: None,
            world: None,
        }
    }

    pub fn with_assets(self, path: impl Into<String>) -> Self {
        Self {
            asset_path: Some(path.into()),
            ..self
        }
    }

    pub fn with_world(self, world: Option<PathBuf>) -> Self {
        Self { world, ..self }
    }
}

impl Plugin for MinecraftServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
            owns_task_pools: self.owns_task_pools,
            asset_path: self.asset_path.clone(),
        });
        let mut world_gen = mcrs_minecraft_worldgen::bevy::WorldGenConfig::default();
        if let Some(world) = &self.world {
            app.insert_resource(WorldSave(world.clone()));
            let settings = mcrs_minecraft_world::save::read_world_gen_settings(world)
                .unwrap_or_else(|err| panic!("{err}"));
            world_gen.seed = settings.seed as u64;
        }
        app.insert_resource(world_gen);
        app.add_plugins(mcrs_minecraft_core::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin);
        app.add_plugins(NetworkPlugin {
            address: self.bind_address,
        });
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(crate::block_light_table::BlockLightTablePlugin);
        app.add_plugins(crate::world::generate::modern_carvers::ModernCarverPlugin);
        app.add_plugins(crate::world::heightmap::HeightmapPredicatesPlugin);
        app.add_plugins(WorldPlugin);
        app.add_plugins(ClientInfoPlugin);
    }
}
