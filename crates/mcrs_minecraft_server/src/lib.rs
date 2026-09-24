#![recursion_limit = "2048"]
#![allow(unexpected_cfgs, clippy::type_complexity, clippy::too_many_arguments)]

extern crate core;

pub mod block_light_table;
mod client_info;
pub mod runner;
pub use runner::{DEFAULT_TPS, run_server_loop};
pub mod configuration;
pub mod dim;
pub mod disconnect;
mod keep_alive;
pub mod login;
pub mod ops;
pub mod world;
pub mod world_options;

use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::LoginPlugin;
use crate::world::WorldPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_level::server_loop::VoxelServerPlugin;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_network::NetworkPlugin;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

pub use mcrs_minecraft_level::server_loop::spawn_server_thread;
pub use mcrs_minecraft_network::BoundAddress;

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
    /// Shared with a client in the same process, whose debug views read each
    /// dimension's column lifecycle from it.
    pub column_traces: Option<ColumnTraceSink>,
    pub lighting: Lighting,
    /// Operator level of a player who has no entry in `ops.json`.
    pub default_op_level: u8,
}

/// Whether dimensions propagate light. Without it the block light table is never built, so
/// no dimension registers a lighting engine and every column goes to the client at full sky
/// light: sunlight, torches and shadows stop existing, and the world loads without the
/// propagation cost.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lighting {
    #[default]
    Propagated,
    FullSky,
}

impl Lighting {
    /// `MCRS_NO_LIGHTING=1` turns propagation off.
    pub fn from_env() -> Self {
        match std::env::var("MCRS_NO_LIGHTING").as_deref() {
            Ok("1" | "true" | "on" | "yes") => Self::FullSky,
            _ => Self::Propagated,
        }
    }
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
            column_traces: None,
            lighting: Lighting::from_env(),
            default_op_level: 0,
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
            ..Default::default()
        }
    }
}

impl Plugin for MinecraftServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
            owns_task_pools: self.owns_task_pools,
            asset_path: self.asset_path.clone(),
        });
        let mut world_seed = crate::world_options::world_seed_from_env();
        if let Some(world) = &self.world {
            app.insert_resource(WorldSave(world.clone()));
            let settings = mcrs_minecraft_world::save::read_world_gen_settings(world)
                .unwrap_or_else(|err| panic!("{err}"));
            world_seed.0 = settings.seed as u64;
        }
        app.insert_resource(world_seed);
        app.insert_resource(self.lighting);
        let ops = ops::OpList::read(std::path::Path::new(ops::OPS_FILE))
            .unwrap_or_else(|err| panic!("{err}"));
        app.insert_resource(ops);
        app.insert_resource(ops::DefaultOpLevel(
            crate::world::entity::player::ability::PlayerOpLevel(self.default_op_level),
        ));
        if let Some(traces) = &self.column_traces {
            app.insert_resource(traces.clone());
        }
        app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin);
        app.add_plugins(NetworkPlugin {
            address: self.bind_address,
        });
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(crate::block_light_table::BlockLightTablePlugin);
        app.add_plugins(crate::world::generate::modern_carvers::ModernCarverPlugin);
        app.add_plugins(crate::world::generate::features::FeaturePlugin);
        app.add_plugins(crate::world::generate::structures::StructurePlugin);
        app.add_plugins(crate::world::heightmap::HeightmapPredicatesPlugin);
        app.add_plugins(WorldPlugin);
        app.add_plugins(ClientInfoPlugin);
    }
}
