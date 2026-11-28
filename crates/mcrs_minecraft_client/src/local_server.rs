use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use bevy::asset::io::file::FileAssetReader;
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use mcrs_minecraft_client_options::VIEW_DISTANCE_DEFAULT;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_client_world::session::{
    LocalServer, LocalServerError, LocalServerPhase, LocalServerStopped, LocalWorldError,
    OpenLocalWorld,
};
use mcrs_minecraft_client_world::world_list::WorldStoreError;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_dimension_environment::environment::WeatherLevel;
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::coordinator::ops::{OpList, OpListEntry};
use mcrs_minecraft_engine_server::login::SingleplayerProfile;
use mcrs_minecraft_engine_server::world_options::WorldLock;
use mcrs_minecraft_engine_server::{
    DataPack, DimensionWorldsPlugin, NetworkCompressionThreshold, ServerCorePlugin,
    ServerNetworkPlugin, ServerThread, ServerThreadError, WorldOpenError,
};
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_network::client::{
    CancelJoin, ConnectionEnded, JoinRequest, offline_player_uuid,
};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registries::{Registries, RegistryAdditions};
use mcrs_minecraft_save::{SaveError, SaveLock, read_weather};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_server_settings::ServerSettings;
use mcrs_minecraft_worldgen::registries::add_vanilla_worldgen;

use crate::config;

#[derive(Clone)]
pub struct LocalServerPlugin {
    pub asset_path: String,
    pub data_pack: PathBuf,
    pub ops_file: Option<PathBuf>,
    pub username: String,
    pub open_to_lan: bool,
    pub settings: ServerSettings,
    pub column_traces: Option<ColumnTraceSink>,
    pub extend_server: fn(&mut App),
}

#[derive(Resource)]
struct Settings(LocalServerPlugin);

#[derive(Resource, Default)]
struct WaitingOpen(Option<OpenLocalWorld>);

#[derive(Resource)]
struct LocalServerThread {
    thread: ServerThread<LocalServerError>,
    host: Uuid,
}

impl Plugin for LocalServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings(self.clone()))
            .init_resource::<WaitingOpen>()
            .add_message::<OpenLocalWorld>()
            .add_message::<LocalServerStopped>()
            .add_message::<CancelJoin>()
            .add_systems(
                Update,
                (
                    open_local_world.run_if(on_message::<OpenLocalWorld>.or_else(ready_to_open)),
                    join_local_server.run_if(starting),
                    stop_local_server.run_if(
                        on_message::<ConnectionEnded>
                            .and_then(resource_exists::<LocalServerThread>)
                            .and_then(joined),
                    ),
                    finish_local_server.run_if(resource_exists::<LocalServerThread>),
                )
                    .chain(),
            )
            .add_systems(
                Last,
                finish_before_exit
                    .run_if(on_message::<AppExit>.and_then(resource_exists::<LocalServerThread>)),
            );
    }
}

fn starting(server: Option<Res<LocalServer>>) -> bool {
    server.is_some_and(|server| server.phase == LocalServerPhase::Starting)
}

fn joined(server: Option<Res<LocalServer>>) -> bool {
    server.is_some_and(|server| server.phase != LocalServerPhase::Starting)
}

fn ready_to_open(waiting: Res<WaitingOpen>, running: Option<Res<LocalServerThread>>) -> bool {
    waiting.0.is_some() && running.is_none()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the server build allocates only on the frame that opens a local world"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
fn open_local_world(
    (settings, running): (Res<Settings>, Option<Res<LocalServerThread>>),
    mut waiting: ResMut<WaitingOpen>,
    mut opens: ResMut<Messages<OpenLocalWorld>>,
    (mut stopped, mut cancels): (MessageWriter<LocalServerStopped>, MessageWriter<CancelJoin>),
    weather: Option<ResMut<WeatherLevel>>,
    mut commands: Commands,
    #[cfg(feature = "test-support")] tap: Option<Res<crate::test_support::CreativeSlotTap>>,
) {
    for open in opens.drain() {
        if waiting.0.is_none() {
            waiting.0 = Some(open);
        } else {
            warn!(world = %open.folder.display(), "another local world is already opening");
            stopped.write(LocalServerStopped(Err(LocalServerError::Busy)));
        }
    }
    if running.is_some() {
        return;
    }
    let Some(open) = waiting.0.take() else {
        return;
    };
    let host = open
        .host
        .unwrap_or_else(|| offline_player_uuid(&settings.0.username));
    info!(world = %open.folder.display(), "opening a local world");
    cancels.write(CancelJoin);
    if let Some(mut weather) = weather {
        weather.set_if_neq(saved_weather(&open.folder));
    }
    let build = build_server(settings.0.clone(), open.folder, open.lock, host);
    #[cfg(feature = "test-support")]
    let build = crate::test_support::tap_server(build, tap.as_deref());
    match ServerThread::start(build) {
        Ok(thread) => {
            commands.insert_resource(LocalServerThread { thread, host });
            commands.insert_resource(LocalServer {
                phase: LocalServerPhase::Starting,
                origin: open.origin,
            });
        }
        Err(error) => {
            let error = thread_error(error);
            error!(%error, "the local server did not start");
            stopped.write(LocalServerStopped(Err(error)));
        }
    }
}

fn saved_weather(folder: &Path) -> WeatherLevel {
    match read_weather(folder) {
        Ok(weather) => WeatherLevel {
            rain: if weather.raining { 1.0 } else { 0.0 },
            thunder: if weather.thundering { 1.0 } else { 0.0 },
        },
        Err(SaveError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            WeatherLevel::default()
        }
        Err(error) => {
            warn!(%error, "the saved weather of the world");
            WeatherLevel::default()
        }
    }
}

fn server_plugins(settings: &LocalServerPlugin) -> MinecraftServerPlugins {
    let defaults = MinecraftServerPlugins::default();
    MinecraftServerPlugins {
        core: ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(settings.asset_path.clone()),
            data_pack: DataPack::Directory(settings.data_pack.clone()),
            ..defaults.core
        },
        ..defaults
    }
}

pub(crate) fn data_pack_registries(
    settings: &LocalServerPlugin,
) -> Result<Registries, WorldStoreError> {
    let core = server_plugins(settings).core;
    let mut additions = RegistryAdditions {
        local_packs: core.local_packs,
        ..RegistryAdditions::default()
    };
    add_vanilla_worldgen(&mut additions).map_err(|error| {
        error!(%error, "the built-in world generation entries");
        WorldStoreError::DataPack
    })?;
    let vanilla = core.data_pack.resolve().map_err(|error| {
        error!(%error, "the data pack");
        WorldStoreError::DataPack
    })?;
    let assets = FileAssetReader::get_base_path().join(core.asset_path.unwrap_or_default());
    load_registries(&DirectoryPacks::new(assets, vanilla), additions)
        .map(|(registries, _)| registries)
        .map_err(|error| {
            error!(%error, "the data pack");
            WorldStoreError::DataPack
        })
}

fn build_server(
    settings: LocalServerPlugin,
    folder: PathBuf,
    lock: Option<SaveLock>,
    host: Uuid,
) -> impl FnOnce() -> Result<App, LocalServerError> + Send + 'static {
    move || {
        let lock = lock
            .map_or_else(|| SaveLock::take(&folder), Ok)
            .map_err(|error| {
                error!(%error, "the world folder is not locked");
                match error {
                    SaveError::Locked { .. } => LocalServerError::Held,
                    SaveError::Io { source, .. } => LocalServerError::Lock(source.kind()),
                    _ => LocalServerError::Lock(std::io::ErrorKind::Other),
                }
            })?;
        let listed = match &settings.ops_file {
            Some(path) => OpList::read_entries(path).map_err(|error| {
                error!(%error, "the operator file");
                LocalServerError::OpsFile
            })?,
            None => Vec::new(),
        };
        let profile = SingleplayerProfile {
            name: settings.username.clone(),
            id: host,
        };
        let base = server_plugins(&settings);
        let plugins = MinecraftServerPlugins {
            worlds: DimensionWorldsPlugin {
                world: Some(folder),
                column_traces: settings.column_traces,
                ops: OpList::new(
                    std::iter::once(OpListEntry {
                        uuid: host,
                        name: settings.username,
                        level: PlayerOpLevel::MAX,
                        bypasses_player_limit: true,
                    })
                    .chain(listed),
                ),
                seed: settings.settings.world_seed,
                preset: settings.settings.world_preset,
                default_game_mode: settings.settings.default_game_mode,
                host: Some(host),
                ..Default::default()
            },
            network: ServerNetworkPlugin {
                bind_address: config::integrated_bind_address(settings.open_to_lan),
                compression_threshold: if settings.open_to_lan {
                    NetworkCompressionThreshold::default()
                } else {
                    NetworkCompressionThreshold(-1)
                },
                singleplayer_profile: Some(profile),
                ..Default::default()
            },
            ..base
        };
        let mut server = App::new();
        server.insert_resource(WorldLock(lock));
        server.add_plugins(plugins);
        (settings.extend_server)(&mut server);
        Ok(server)
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the username moves into the join request, which is written once when the local server starts"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn join_local_server(
    mut server: ResMut<LocalServerThread>,
    local: ResMut<LocalServer>,
    settings: Res<Settings>,
    view_distance: Option<Res<RequestedViewDistance>>,
    mut joins: MessageWriter<JoinRequest>,
) {
    let Some(address) = server.thread.address() else {
        return;
    };
    info!(%address, "hosting a local server");
    joins.write(JoinRequest {
        server: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), address.port()),
        username: settings.0.username.clone(),
        profile_id: Some(server.host),
        view_distance: view_distance.map_or(VIEW_DISTANCE_DEFAULT, |requested| requested.0),
    });
    local
        .map_unchanged(|local| &mut local.phase)
        .set_if_neq(LocalServerPhase::Running);
}

fn stop_local_server(mut server: ResMut<LocalServerThread>, local: ResMut<LocalServer>) {
    server.thread.request_shutdown();
    local
        .map_unchanged(|local| &mut local.phase)
        .set_if_neq(LocalServerPhase::Stopping);
}

fn finish_local_server(
    mut server: ResMut<LocalServerThread>,
    mut stopped: MessageWriter<LocalServerStopped>,
    weather: Option<ResMut<WeatherLevel>>,
    waiting: Res<WaitingOpen>,
    mut commands: Commands,
) {
    let Some(exit) = server.thread.try_finish() else {
        return;
    };
    let result = match exit {
        Ok(AppExit::Success) => Ok(()),
        Ok(AppExit::Error(code)) => Err(LocalServerError::Exited(code.get())),
        Err(error) => Err(thread_error(error)),
    };
    match &result {
        Ok(()) => info!("the local server saved the world and stopped"),
        Err(error) => error!(%error, "the local server stopped"),
    }
    if let Some(mut weather) = weather {
        weather.set_if_neq(WeatherLevel::default());
    }
    commands.remove_resource::<LocalServerThread>();
    commands.remove_resource::<LocalServer>();
    if result.is_err() || waiting.0.is_none() {
        stopped.write(LocalServerStopped(result));
    }
}

fn finish_before_exit(mut server: ResMut<LocalServerThread>, _main_thread: NonSendMarker) {
    info!("waiting for the local server to save the world");
    match server.thread.finish() {
        Some(Ok(AppExit::Success)) => info!("the local server saved the world and stopped"),
        Some(Ok(AppExit::Error(code))) => error!(code, "the local server stopped with an error"),
        Some(Err(error)) => error!(error = %thread_error(error), "the local server stopped"),
        None => {}
    }
}

fn thread_error(error: ServerThreadError<LocalServerError>) -> LocalServerError {
    if let ServerThreadError::World(error) = &error {
        error!(%error, "the local world did not open");
    }
    match error {
        ServerThreadError::Build(error) => error,
        ServerThreadError::Spawn(kind) => LocalServerError::Spawn(kind),
        ServerThreadError::NotBound => LocalServerError::NotBound,
        ServerThreadError::World(WorldOpenError::Locked { .. }) => LocalServerError::Held,
        ServerThreadError::World(WorldOpenError::DataVersion {
            found, expected, ..
        }) => LocalServerError::World(LocalWorldError::DataVersion { found, expected }),
        ServerThreadError::World(WorldOpenError::Save(_)) => {
            LocalServerError::World(LocalWorldError::Files)
        }
        ServerThreadError::World(WorldOpenError::Dimensions(_)) => {
            LocalServerError::World(LocalWorldError::Dimensions)
        }
        _ => LocalServerError::Panicked,
    }
}
