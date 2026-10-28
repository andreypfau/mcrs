use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use bevy::asset::AssetPlugin;
use bevy::asset::io::AssetSourceId;
#[cfg(not(target_family = "wasm"))]
use bevy::asset::io::file::FileAssetReader;
use bevy::log::{BoxedLayer, LogPlugin};
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::WgpuSettings;
use bevy::transform::TransformSystems;
use bevy::window::{
    Monitor, MonitorSelection, PresentMode, PrimaryMonitor, WindowLevel, WindowMode,
    WindowPosition, WindowResolution,
};
use bevy::winit::{UpdateMode, WinitSettings};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::Id;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::Weather;
use mcrs_minecraft_environment::world_clock::{AdvanceTime, WorldClocks};
use mcrs_minecraft_level::entity::physics::Transform as PhysicsTransform;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registries::CurrentRegistries;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_world::save::{self, SaveError};

#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_client::columns::SECTION_SIZE;
use mcrs_minecraft_client::config::{Knobs, TerrainLimits};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_client::registries::ClientRegistriesPlugin;
use mcrs_minecraft_client::registries::SessionSet;
use mcrs_minecraft_client::{
    ClientPlugins, ClientTerrainPlugin, config, gui, local_player, player, sky, vanilla,
};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_client::{asset_source, screenshot};
use mcrs_minecraft_environment::world_clock::WorldClock;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_network::client::offline_player_uuid;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_network::client::{ClientNetworkPlugin, ExitOnDisconnect};
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_server::login::SingleplayerProfile;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_server::{
    BoundAddress, DataPack, DimensionWorldsPlugin, LightingPlugin, MinecraftServerPlugins,
    NetworkCompressionThreshold, ServerCorePlugin, ServerNetworkPlugin,
    ops::{OPS_FILE, OpList, OpListEntry},
};

#[cfg(feature = "telemetry-tracy")]
#[global_allocator]
static ALLOC: tracing_tracy::client::ProfiledAllocator<std::alloc::System> =
    tracing_tracy::client::ProfiledAllocator::new(std::alloc::System, 8);

#[cfg(feature = "telemetry-tracy")]
fn tracy_layer(_: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(tracing_tracy::TracyLayer::default()))
}

#[cfg(not(feature = "telemetry-tracy"))]
fn tracy_layer(_: &mut App) -> Option<BoxedLayer> {
    None
}

/// Tracy draws its frame boundaries from this. `bevy_render` marks them with a
/// tracing event, but only the profiler's own subscriber filters that event
/// back out of the log, so going through the client directly is what keeps a
/// profiled run from writing a line per frame.
#[cfg(feature = "telemetry-tracy")]
fn frame_mark() {
    tracing_tracy::client::frame_mark();
}

/// The browser has no world folder and no save to seed itself from, so the
/// entry point there is its own.
#[cfg(target_family = "wasm")]
fn main() {
    mcrs_minecraft_client::web::run();
}

#[cfg(not(target_family = "wasm"))]
#[expect(
    clippy::too_many_lines,
    reason = "the client's entry point builds the whole app, and its plugins, resources and systems read top to bottom in the order they are added"
)]
fn main() -> AppExit {
    #[cfg(target_os = "macos")]
    mcrs_minecraft_client::app_nap::decline();
    let knobs = knobs();
    let world = world_folder();
    let save_data = world.as_deref().map(load_save).unwrap_or_default();
    let frozen_at = knobs.frozen_time();
    let resolution = knobs.resolution();
    let view_distance = knobs.view_distance();
    let assets = AssetPlugin::default().file_path;
    // The client cannot run without the jar, so the first start fetches it before the window
    // opens and the resource thread then finds it installed.
    let data_pack = match mcrs_minecraft_client_jar::vanilla_data(
        &mcrs_minecraft_client_jar::ClientJar::InstalledOrDownloaded,
        None,
        &mcrs_minecraft_client_jar::Progress::default(),
    ) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("error: the vanilla data pack: {error}");
            return AppExit::error();
        }
    };

    let wgpu = WgpuSettings {
        features: WgpuFeatures::TIMESTAMP_QUERY,
        ..default()
    };
    // Bevy keeps wgpu's per-draw validation pass whenever DX12 is among the backends, which the
    // default set is even where DX12 cannot exist; the cull writes every indirect argument itself.
    // Labels handed to Metal cost a fifth of the frame's encoding. The environment still wins,
    // so `WGPU_DISCARD_HAL_LABELS=0` brings them back for a GPU capture.
    #[cfg(not(debug_assertions))]
    let wgpu = {
        use bevy::render::settings::InstanceFlags;
        let mut wgpu = wgpu;
        #[cfg(not(target_os = "windows"))]
        wgpu.instance_flags
            .remove(InstanceFlags::VALIDATION_INDIRECT_CALL);
        wgpu.instance_flags = (wgpu.instance_flags | InstanceFlags::DISCARD_HAL_LABELS).with_env();
        wgpu
    };
    let mut task_pool_options = bevy::app::TaskPoolOptions::default();
    task_pool_options.async_compute.max_threads = knobs.async_threads();
    task_pool_options.async_compute.percent = 1.0;
    task_pool_options.io.max_threads = config::IO_THREADS;
    let mut app = App::new();
    app.insert_resource(knobs.clone());
    app.register_asset_source(
        AssetSourceId::Default,
        asset_source(&PackRoots {
            assets: assets.clone(),
            vanilla: data_pack.clone(),
        }),
    );
    vanilla::register(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(bevy::app::TaskPoolPlugin { task_pool_options })
            .set(AssetPlugin {
                file_path: assets.clone(),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: wgpu.into(),
                ..default()
            })
            .set(LogPlugin {
                custom_layer: tracy_layer,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: match &world {
                        Some(world) => format!("mcrs — {}", world.display()),
                        None => "mcrs".to_owned(),
                    },
                    mode: if knobs.fullscreen() && resolution.is_none() {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                    } else {
                        WindowMode::Windowed
                    },
                    position: WindowPosition::Centered(MonitorSelection::Primary),
                    // A window another one covers stops being presented, and a frame that gets no
                    // swapchain texture is a frame that is never drawn, so a sized window stays on
                    // top for the length of the measurement.
                    window_level: if resolution.is_some() {
                        WindowLevel::AlwaysOnTop
                    } else {
                        WindowLevel::Normal
                    },
                    resolution: match resolution {
                        Some((width, height)) => {
                            WindowResolution::new(width, height).with_scale_factor_override(1.0)
                        }
                        None => WindowResolution::default(),
                    },
                    present_mode: if knobs.vsync() {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    },
                    #[cfg(feature = "dev")]
                    desired_maximum_frame_latency: knobs.frame_latency(),
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::pbr::PbrPlugin>()
            .disable::<bevy::light::LightPlugin>(),
    )
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    })
    .add_plugins(
        ClientPlugins
            .build()
            .set(ClientRegistriesPlugin {
                local_corpus: Some(DirectoryPacks::new(
                    FileAssetReader::get_base_path().join(&assets),
                    data_pack.clone(),
                )),
            })
            .add_before::<sky::SkyPlugin>(
                mcrs_minecraft_client::item_model::resolve::ItemRenderPlugin,
            )
            .add_before::<sky::SkyPlugin>(gui::scene::GuiPlugin)
            .add(screenshot::ScreenshotPlugin),
    )
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .add_systems(
        Update,
        (log_registry_counts, log_seeded_resources)
            .after(apply_saved_clocks)
            .run_if(resource_exists_and_changed::<CurrentRegistries>),
    )
    .add_systems(
        PostStartup,
        (
            log_spawned_transforms.after(TransformSystems::Propagate),
            log_monitors,
        ),
    );

    // After `DefaultPlugins`: an embedded server leaves the task pools to its
    // host, so the host has to have built them before the server thread ticks.
    let username = knobs.username();
    #[cfg(feature = "singleplayer")]
    let server = server_address(&knobs).unwrap_or_else(|| {
        #[cfg(feature = "dev")]
        let traces = {
            let traces = ColumnTraceSink::default();
            app.insert_resource(traces.clone());
            Some(traces)
        };
        #[cfg(not(feature = "dev"))]
        let traces = None;
        let host = SingleplayerProfile {
            name: username.clone(),
            id: save_data
                .player_uuid
                .unwrap_or_else(|| offline_player_uuid(&username)),
        };
        host_integrated_server(&knobs, world.as_deref(), &assets, &data_pack, traces, host)
    });
    #[cfg(not(feature = "singleplayer"))]
    let server = server_address(&knobs)
        .expect("a client built without singleplayer hosts no server; set MCRS_SERVER");
    app.add_plugins(ClientNetworkPlugin {
        server,
        username,
        profile_id: save_data.player_uuid,
        view_distance,
        holds_vanilla_pack: true,
    });
    app.add_plugins(mcrs_minecraft_client::columns::ColumnCachePlugin);
    app.init_resource::<mcrs_minecraft_client::screen::Screen>();
    app.add_plugins(mcrs_minecraft_client::game_mode::GameModePlugin);
    app.add_plugins(gui::game_mode_switcher::GameModeSwitcherPlugin);
    app.add_plugins(mcrs_minecraft_client::chat::ChatPlugin {
        command_history: Some(std::path::PathBuf::from("command_history.txt")),
    });
    app.add_plugins(gui::chat_input::ChatScreenPlugin);
    app.insert_resource(ExitOnDisconnect);

    let mut saved_clocks = save_data.world_clocks;
    if let Some(ticks) = frozen_at {
        for state in saved_clocks.values_mut() {
            state.total_ticks = ticks;
            state.partial_tick = 0.0;
        }
    }
    app.insert_resource(SavedClocks(saved_clocks))
        .add_systems(
            Update,
            apply_saved_clocks.after(SessionSet::Derive).run_if(
                resource_exists::<SavedClocks>.and_then(resource_exists::<CurrentRegistries>),
            ),
        )
        .insert_resource(AdvanceTime(save_data.advance_time && frozen_at.is_none()))
        .insert_resource(save_data.weather);

    app.add_plugins(ClientTerrainPlugin(terrain_limits(view_distance)));
    #[cfg(feature = "dev")]
    app.add_plugins(mcrs_minecraft_client::dev::DevPlugin);

    #[cfg(feature = "telemetry-tracy")]
    app.add_systems(Last, frame_mark);

    let look = knobs.look_override();
    let (yaw, pitch) = look.unwrap_or((save_data.yaw, save_data.pitch));
    if look.is_some() {
        app.insert_resource(local_player::LookOverride { yaw, pitch });
    }
    let position = knobs.position_override();
    if let Some(position) = position {
        app.insert_resource(local_player::PositionOverride(position));
    }
    let player = player::spawn_player(
        app.world_mut(),
        position.unwrap_or(save_data.position),
        yaw,
        pitch,
    );
    #[cfg(feature = "dev")]
    mcrs_minecraft_client::dev::script_flight(&mut app, player);
    #[cfg(not(feature = "dev"))]
    let _ = player;

    app.run()
}

fn log_monitors(monitors: Query<(&Monitor, Has<PrimaryMonitor>)>) {
    for (monitor, primary) in &monitors {
        info!(
            name = monitor.name.as_deref().unwrap_or("?"),
            width = monitor.physical_width,
            height = monitor.physical_height,
            hz = monitor.refresh_rate_millihertz.map(|hz| hz as f32 / 1000.0),
            scale = monitor.scale_factor,
            primary,
            "monitor"
        );
    }
}

/// One block holds every group of every bucket and a flush takes a fresh one before freeing the
/// stale one, so the arena has to fit two of them with the buddy rounding on top.
///
/// The tint window covers the view and two columns past it on each side, which a column the
/// client has yet to drop can still stand in.
fn terrain_limits(view_distance: u8) -> TerrainLimits {
    TerrainLimits {
        arena_scale: 4,
        groups: 1 << 23,
        sections: 1 << 19,
        tint_span: (2 * (u32::from(view_distance) + 2) + 1) * SECTION_SIZE as u32,
    }
}

/// Singleplayer, the way the vanilla client plays it: a server of our own, which the client then
/// joins over loopback like any other. It listens on loopback only, unless `MCRS_OPEN_TO_LAN=1`
/// opens it to the local network, where it then announces itself.
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
fn host_integrated_server(
    knobs: &Knobs,
    world: Option<&Path>,
    assets: &str,
    data_pack: &Path,
    traces: Option<ColumnTraceSink>,
    host: SingleplayerProfile,
) -> SocketAddr {
    let open_to_lan = knobs.open_to_lan();
    let mut server = App::new();
    let mut plugins = MinecraftServerPlugins {
        core: ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(assets.to_owned()),
            data_pack: DataPack::Directory(data_pack.to_owned()),
        },
        worlds: DimensionWorldsPlugin {
            world: world.map(Path::to_path_buf),
            column_traces: traces,
            // The host of a singleplayer world is an operator at the highest level, unless the
            // ops file says otherwise: a later entry for the same player replaces it.
            ops: OpList::new(
                std::iter::once(OpListEntry {
                    uuid: host.id,
                    name: host.name.clone(),
                    level:
                        mcrs_minecraft_server::world::entity::player::ability::PlayerOpLevel::MAX,
                    bypasses_player_limit: true,
                })
                .chain(OpList::read_entries(Path::new(OPS_FILE)).unwrap_or_else(
                    |error| {
                        eprintln!("{error}");
                        std::process::exit(1);
                    },
                )),
            ),
            seed: knobs.world_seed(),
            preset: knobs.world_preset(),
            default_game_mode: knobs.default_game_mode(),
            slow_column_threshold: knobs.slow_column_threshold(),
            ..Default::default()
        },
        network: ServerNetworkPlugin {
            bind_address: config::integrated_bind_address(open_to_lan),
            // The host shares this process, so only LAN guests gain from compression.
            compression_threshold: if open_to_lan {
                NetworkCompressionThreshold::default()
            } else {
                NetworkCompressionThreshold(-1)
            },
            singleplayer_profile: Some(host),
            ..Default::default()
        },
    }
    .build();
    if !knobs.propagates_light() {
        plugins = plugins.disable::<LightingPlugin>();
    }
    server.add_plugins(plugins);
    let address = server.world().resource::<BoundAddress>().0;
    let spawned = mcrs_minecraft_server::spawn_server_thread(server, |app| {
        if let AppExit::Error(code) = mcrs_minecraft_server::run_server_loop(app) {
            error!("the integrated server stopped with an error");
            std::process::exit(i32::from(code.get()));
        }
    });
    if let Err(error) = spawned {
        error!(%error, "the integrated server thread did not start");
        std::process::exit(1);
    }
    match world {
        Some(world) => info!(
            world = %world.display(),
            %address,
            "hosting an integrated server, which reads this world's saved chunks and \
             generates the columns it has never saved",
        ),
        None => info!(%address, "hosting an integrated server"),
    }
    SocketAddr::new(std::net::Ipv4Addr::LOCALHOST.into(), address.port())
}

/// `MCRS_SERVER=<host>:<port>` joins that server instead of hosting one.
fn server_address(knobs: &Knobs) -> Option<SocketAddr> {
    let address = knobs.server()?;
    match address.to_socket_addrs().map(|mut a| a.next()) {
        Ok(Some(address)) => Some(address),
        Ok(None) | Err(_) => {
            eprintln!("MCRS_SERVER={address}: expected <host>:<port>");
            std::process::exit(1);
        }
    }
}

/// The `MCRS_` variables of the process environment, and whether Metal's HUD times encoders.
#[cfg(not(target_family = "wasm"))]
#[expect(
    clippy::disallowed_methods,
    reason = "the client binary reads its settings from the environment"
)]
fn knobs() -> Knobs {
    let variables = std::env::vars_os().filter_map(|(name, value)| {
        let name = name.into_string().ok()?;
        // Windows matches variable names in any case, as `std::env::var` does there.
        #[cfg(windows)]
        let name = name.to_ascii_uppercase();
        Some((
            name.strip_prefix("MCRS_")?.to_owned(),
            value.into_string().ok()?,
        ))
    });
    Knobs::new(
        variables,
        std::env::var("MTL_HUD_ENCODER_TIMING_ENABLED").is_ok_and(|on| on != "0"),
    )
}

/// A world folder seeds the clocks, the weather and the spawn, and gives the
/// integrated server its saved chunks. Without one the server generates.
fn world_folder() -> Option<PathBuf> {
    let path = std::env::args_os().nth(1).map(PathBuf::from)?;
    if !path.is_dir() {
        eprintln!("not a world folder: {}", path.display());
        std::process::exit(1);
    }
    Some(path)
}

#[cfg(not(target_family = "wasm"))]
struct SaveData {
    world_clocks: save::WorldClockStates,
    player_uuid: Option<Uuid>,
    advance_time: bool,
    weather: Weather,
    position: DVec3,
    yaw: f32,
    pitch: f32,
}

#[cfg(not(target_family = "wasm"))]
impl Default for SaveData {
    fn default() -> Self {
        Self {
            world_clocks: save::WorldClockStates::default(),
            player_uuid: None,
            advance_time: true,
            weather: Weather::default(),
            position: DVec3::new(0.5, 80.0, 0.5),
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
fn load_save(world: &Path) -> SaveData {
    let level = save::read_level_dat(world).unwrap_or_else(|err| fatal(err));
    let world_clocks = save::read_world_clocks(world).unwrap_or_else(|err| fatal(err));
    let weather = save::read_weather(world).unwrap_or_else(|err| fatal(err));
    let game_rules = save::read_game_rules(world).unwrap_or_else(|err| fatal(err));

    let (position, yaw, pitch) = match level.singleplayer_uuid {
        Some(uuid) => match save::read_player_dat(world, uuid) {
            Ok(Some(player)) => (
                DVec3::from_array(player.pos),
                player.rotation[0],
                player.rotation[1],
            ),
            Ok(None) => spawn_fallback(&level.spawn),
            Err(err) => fatal(err),
        },
        None => spawn_fallback(&level.spawn),
    };

    SaveData {
        world_clocks,
        player_uuid: level.singleplayer_uuid,
        advance_time: game_rules.advance_time,
        weather: Weather {
            rain: if weather.raining { 1.0 } else { 0.0 },
            thunder: if weather.thundering { 1.0 } else { 0.0 },
        },
        position,
        yaw,
        pitch,
    }
}

/// A spawn point is a block position; the player stands at its centre in X and
/// Z, and Y is the block's own floor.
#[cfg(not(target_family = "wasm"))]
fn spawn_fallback(spawn: &save::RespawnData) -> (DVec3, f32, f32) {
    (
        DVec3::new(
            spawn.pos[0] as f64 + 0.5,
            spawn.pos[1] as f64,
            spawn.pos[2] as f64 + 0.5,
        ),
        spawn.yaw,
        spawn.pitch,
    )
}

#[cfg(not(target_family = "wasm"))]
fn fatal(err: SaveError) -> ! {
    eprintln!("{err}");
    std::process::exit(1);
}

#[derive(Resource)]
struct SavedClocks(save::WorldClockStates);

fn apply_saved_clocks(
    saved: Res<SavedClocks>,
    registries: Res<CurrentRegistries>,
    mut clocks: ResMut<WorldClocks>,
    mut commands: Commands,
) {
    if let Some(registry) = registries.registry::<WorldClock>() {
        for (name, state) in &saved.0 {
            match registry
                .by_name(name)
                .and_then(|id| Id::checked(id.number(), registry.len()))
            {
                Some(id) => clocks.insert(id, *state),
                None => {
                    warn!(clock = %name, "discarding saved clock state with no world_clock registry entry")
                }
            }
        }
    }
    commands.remove_resource::<SavedClocks>();
}

fn log_registry_counts(registries: Res<CurrentRegistries>) {
    fn count<V: mcrs_minecraft_core::RegistryValue + Send + Sync + 'static>(
        registries: &CurrentRegistries,
    ) -> usize {
        registries
            .registry::<V>()
            .map_or(0, |registry| registry.len())
    }
    info!(
        dimension_types = count::<DimensionTypeFile>(&registries),
        biomes = count::<BiomeFile>(&registries),
        timelines = count::<mcrs_minecraft_environment::timeline::Timeline>(&registries),
        world_clocks = count::<WorldClock>(&registries),
        "registry assets loaded"
    );
}

fn log_spawned_transforms(
    player: Single<(&Transform, &PhysicsTransform), With<player::Player>>,
    camera: Single<&GlobalTransform, With<player::PlayerCamera>>,
) {
    let (transform, physics) = player.into_inner();
    info!(
        player_translation = ?transform.translation,
        camera_world_translation = ?camera.translation(),
        yaw = physics.rotation.yaw(),
        pitch = physics.rotation.pitch(),
        "spawned player"
    );
}

fn log_seeded_resources(
    clocks: Res<WorldClocks>,
    registries: Res<CurrentRegistries>,
    advance_time: Res<AdvanceTime>,
    weather: Res<Weather>,
) {
    let names = registries.registry::<WorldClock>();
    info!(
        clocks = ?clocks
            .iter()
            .map(|(id, state)| {
                let name = names.and_then(|names| {
                    names.name(Id::checked(id.number(), names.len())?)
                });
                (name.map(ToString::to_string), state.total_ticks)
            })
            .collect::<Vec<_>>(),
        advance_time = advance_time.0,
        rain = weather.rain,
        thunder = weather.thunder,
        "seeded resources from save"
    );
}
