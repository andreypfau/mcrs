#![recursion_limit = "2048"]

extern crate core;

pub mod block_light_table;
mod client_info;
pub mod runner;
pub use runner::{DEFAULT_TPS, run_server_loop};
pub mod configuration;
mod keep_alive;
pub mod login;
pub mod ops;
mod status;
pub mod world;
pub mod world_options;

pub use crate::block_light_table::LightingPlugin;
use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::{LoginPlugin, SingleplayerProfile};
use crate::status::StatusPlugin;
use crate::world::WorldPlugin;
pub use crate::world::generate::WorldGenPlugin;
use bevy_app::{App, AppExit, Plugin, PluginGroup, PluginGroupBuilder};
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_level::server_loop::VoxelServerPlugin;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_network::NetworkPlugin;
use mcrs_minecraft_world::dimension::Dimensions;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::time::Duration;

pub use login::NetworkCompressionThreshold;
#[cfg(feature = "client-jar")]
pub use mcrs_minecraft_client_jar::ClientJar;
pub use mcrs_minecraft_game_mode::GameMode;
pub use mcrs_minecraft_level::server_loop::spawn_server_thread;
pub use mcrs_minecraft_network::BoundAddress;

/// Where the server reads the game's data pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataPack {
    /// A directory laid out as the client jar's `data/minecraft` tree with its `version.json`.
    /// A relative path is resolved as the asset root is.
    Directory(PathBuf),
    /// The pinned release's client jar, unpacked once below `cache`, or the user's cache
    /// directory when it is `None`.
    #[cfg(feature = "client-jar")]
    ClientJar {
        jar: ClientJar,
        cache: Option<PathBuf>,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DataPackError {
    #[cfg(feature = "client-jar")]
    #[error("the vanilla data pack: {0}")]
    ClientJar(#[from] mcrs_minecraft_client_jar::VanillaDataError),
}

impl DataPack {
    /// The directory, unpacking the client jar first when that is the source.
    pub fn resolve(&self) -> Result<PathBuf, DataPackError> {
        match self {
            DataPack::Directory(dir) => Ok(dir.clone()),
            #[cfg(feature = "client-jar")]
            DataPack::ClientJar { jar, cache } => Ok(mcrs_minecraft_client_jar::vanilla_data(
                jar,
                cache.as_deref(),
                &mcrs_minecraft_client_jar::Progress::default(),
            )?),
        }
    }
}

impl Default for DataPack {
    #[cfg(feature = "client-jar")]
    fn default() -> Self {
        DataPack::ClientJar {
            jar: ClientJar::InstalledOrDownloaded,
            cache: None,
        }
    }

    #[cfg(not(feature = "client-jar"))]
    fn default() -> Self {
        DataPack::Directory(PathBuf::from("assets/minecraft"))
    }
}

/// Every part of the server, each one a member that can be disabled or replaced through
/// [`PluginGroup::build`]. A member that needs another one panics at build and names it.
#[derive(Default)]
pub struct MinecraftServerPlugins {
    pub core: ServerCorePlugin,
    pub worlds: DimensionWorldsPlugin,
    pub network: ServerNetworkPlugin,
}

/// The world folder the server reads its saved chunks from.
#[derive(Resource, Clone)]
pub struct WorldSave(pub PathBuf);

impl MinecraftServerPlugins {
    /// A server running inside another process: loopback only, an OS-assigned
    /// port, and the task pools left to the host app.
    pub fn embedded() -> Self {
        Self {
            core: ServerCorePlugin {
                owns_task_pools: false,
                ..Default::default()
            },
            network: ServerNetworkPlugin {
                bind_address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
                // Both ends share one process, so compression only spends CPU on each side.
                compression_threshold: NetworkCompressionThreshold(-1),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

impl PluginGroup for MinecraftServerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(self.core)
            .add(mcrs_minecraft_command_engine::CommandPlugin::<
                mcrs_minecraft_command_vanilla::VanillaCommands,
            >::new())
            .add(mcrs_minecraft_command_vanilla::VanillaCommandsPlugin)
            .add(self.worlds)
            .add(WorldGenPlugin)
            .add(LightingPlugin)
            .add(self.network)
    }
}

/// Panics unless `P` is built, so a missing member is named at build and not as a missing
/// resource at the first tick.
pub(crate) fn require<By: Plugin, P: Plugin>(app: &App) {
    assert!(
        app.is_plugin_added::<P>(),
        "{} needs {} added before it",
        std::any::type_name::<By>(),
        std::any::type_name::<P>()
    );
}

/// The tick, the task pools, the asset server, and the registries with the block and item
/// definitions.
pub struct ServerCorePlugin {
    /// Clear when the server shares a process with another Bevy app, which
    /// then owns the process-global task pools.
    pub owns_task_pools: bool,
    /// Absolute path to the asset corpus, for a server that cannot resolve
    /// `assets` from its own working directory or executable location.
    pub asset_path: Option<String>,
    /// Where the game's data pack is read from.
    pub data_pack: DataPack,
}

impl Default for ServerCorePlugin {
    fn default() -> Self {
        Self {
            owns_task_pools: true,
            asset_path: None,
            data_pack: DataPack::default(),
        }
    }
}

impl Plugin for ServerCorePlugin {
    fn build(&self, app: &mut App) {
        let vanilla = match self.data_pack.resolve() {
            Ok(vanilla) => vanilla,
            Err(error) => {
                tracing::error!("the registries are unusable:\n{error}");
                app.world_mut().write_message(AppExit::error());
                return;
            }
        };
        let roots = mcrs_minecraft_assets::packs::PackRoots {
            assets: self
                .asset_path
                .clone()
                .unwrap_or_else(|| bevy_asset::AssetPlugin::default().file_path),
            vanilla,
        };
        mcrs_minecraft_assets::packs::register_pack_sources(
            app,
            &roots,
            mcrs_minecraft_worldgen_builtin::asset,
        );
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
            owns_task_pools: self.owns_task_pools,
            asset_path: self.asset_path.clone(),
        });
        app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin);
        app.add_plugins(mcrs_minecraft_worldgen::plugin::VanillaWorldgenPlugin);
    }
}

/// The dimension list and one world per dimension, with the bridge that carries each
/// player's packets between the network and the player's dimension.
pub struct DimensionWorldsPlugin {
    /// World folder to read saved chunks from. Without one, and for any column
    /// the folder has never saved, the dimension generates its terrain.
    pub world: Option<PathBuf>,
    /// Shared with a client in the same process, whose debug views read each
    /// dimension's column lifecycle from it.
    pub column_traces: Option<ColumnTraceSink>,
    /// The seed of a world without a save; a save's own seed replaces it.
    pub seed: u64,
    /// The world preset that names the dimensions when neither the save nor `dimensions` does.
    pub preset: String,
    /// The dimensions the world spawns, in place of a save's list and the world preset; the
    /// overworld is then optional, and a player with no saved dimension spawns in the first.
    pub dimensions: Option<Dimensions>,
    pub ops: ops::OpList,
    /// Operator level of a player who has no entry in `ops`.
    pub default_op_level: u8,
    /// Game mode of a player joining for the first time.
    pub default_game_mode: GameMode,
    /// A column that takes longer than this from request to delivery is logged.
    pub slow_column_threshold: Duration,
}

impl Default for DimensionWorldsPlugin {
    fn default() -> Self {
        Self {
            world: None,
            column_traces: None,
            seed: 0,
            preset: "normal".to_owned(),
            dimensions: None,
            ops: ops::OpList::default(),
            default_op_level: 0,
            default_game_mode: GameMode::Creative,
            slow_column_threshold: Duration::from_millis(250),
        }
    }
}

impl Plugin for DimensionWorldsPlugin {
    fn build(&self, app: &mut App) {
        require::<Self, ServerCorePlugin>(app);
        app.insert_resource(crate::world_options::WorldSeed(self.seed));
        app.insert_resource(crate::world_options::WorldPresetName(self.preset.clone()));
        if let Some(world) = &self.world {
            app.insert_resource(WorldSave(world.clone()));
        }
        if let Some(dimensions) = &self.dimensions {
            app.insert_resource(crate::world_options::PluginDimensions(dimensions.clone()));
        }
        app.insert_resource(self.ops.clone());
        app.add_observer(ops::broadcast_to_operators);
        app.insert_resource(ops::DefaultOpLevel(
            mcrs_minecraft_engine::ability::PlayerOpLevel(self.default_op_level),
        ));
        app.insert_resource(crate::world::entity::player::DefaultGameMode(
            self.default_game_mode,
        ));
        app.insert_resource(crate::world::chunk::SlowColumnThreshold(
            self.slow_column_threshold,
        ));
        if let Some(traces) = &self.column_traces {
            app.insert_resource(traces.clone());
        }
        app.add_systems(bevy_app::Startup, crate::world_options::bake_dimensions);
        app.add_plugins(mcrs_minecraft_engine::heightmap::HeightmapPredicatesPlugin);
        app.add_plugins(WorldPlugin);
    }
}

/// The listener, login, configuration and keep-alive of every connection.
pub struct ServerNetworkPlugin {
    /// Port 0 asks the OS for a free port; read the result back from
    /// [`BoundAddress`].
    pub bind_address: SocketAddr,
    /// A server that listens beyond loopback announces itself on the local network.
    pub announce_on_lan: bool,
    /// Turned off, the server offers no known pack, so a client receives every registry entry's
    /// data instead of only the entries that are not in the vanilla pack.
    pub offer_known_packs: bool,
    pub compression_threshold: NetworkCompressionThreshold,
    pub singleplayer_profile: Option<SingleplayerProfile>,
    pub max_players: u32,
    pub motd: String,
}

impl Default for ServerNetworkPlugin {
    fn default() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 25565).into(),
            announce_on_lan: true,
            offer_known_packs: true,
            compression_threshold: NetworkCompressionThreshold::default(),
            singleplayer_profile: None,
            max_players: 20,
            motd: "mcrs Server".to_owned(),
        }
    }
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        require::<Self, DimensionWorldsPlugin>(app);
        app.insert_resource(crate::configuration::KnownPackOffer(self.offer_known_packs));
        if let Some(host) = &self.singleplayer_profile {
            app.insert_resource(host.clone());
        }
        app.add_plugins(NetworkPlugin {
            address: self.bind_address,
            announce_on_lan: self.announce_on_lan,
            max_players: self.max_players,
            motd: self.motd.clone(),
        });
        app.insert_resource(self.compression_threshold);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(ClientInfoPlugin);
        app.add_plugins(StatusPlugin);
    }
}
