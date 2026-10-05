#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use bevy::asset::AssetPlugin;
use bevy::asset::io::AssetSourceId;
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
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::layered_file_source;
use mcrs_minecraft_dimension::environment::Weather;
use mcrs_minecraft_environment::world_clock::{AdvanceTime, WorldClocks};
use mcrs_minecraft_level::entity::physics::Transform as PhysicsTransform;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::RegistrySet;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_world::save::{self, SaveError};

use mcrs_minecraft_client::columns::SECTION_SIZE;
use mcrs_minecraft_client::config::TerrainLimits;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_client::screenshot;
use mcrs_minecraft_client::{
    ClientPlugins, ClientTerrainPlugin, asset_corpus, config, gui, local_player, player, sky,
    vanilla,
};
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_network::client::offline_player_uuid;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_network::client::{ClientNetworkPlugin, ExitOnDisconnect};
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_server::login::SingleplayerProfile;
#[cfg(all(feature = "singleplayer", not(target_family = "wasm")))]
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugin};

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
fn main() -> AppExit {
    #[cfg(target_os = "macos")]
    mcrs_minecraft_client::app_nap::decline();
    let world = world_folder();
    let save_data = world.as_deref().map(load_save).unwrap_or_default();
    let frozen_at = config::frozen_time();
    let assets = asset_corpus().to_string_lossy().into_owned();

    let mut wgpu = WgpuSettings {
        features: WgpuFeatures::TIMESTAMP_QUERY,
        ..default()
    };
    // Bevy keeps wgpu's per-draw validation pass whenever DX12 is among the backends, which the
    // default set is even where DX12 cannot exist; the cull writes every indirect argument itself.
    // Labels handed to Metal cost a fifth of the frame's encoding. The environment still wins,
    // so `WGPU_DISCARD_HAL_LABELS=0` brings them back for a GPU capture.
    #[cfg(not(debug_assertions))]
    {
        use bevy::render::settings::InstanceFlags;
        #[cfg(not(target_os = "windows"))]
        wgpu.instance_flags
            .remove(InstanceFlags::VALIDATION_INDIRECT_CALL);
        wgpu.instance_flags = (wgpu.instance_flags | InstanceFlags::DISCARD_HAL_LABELS).with_env();
    }
    let mut task_pool_options = bevy::app::TaskPoolOptions::default();
    task_pool_options.async_compute.max_threads = config::async_threads();
    task_pool_options.async_compute.percent = 1.0;
    task_pool_options.io.max_threads = config::IO_THREADS;
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(&assets, mcrs_minecraft_worldgen_builtin::asset),
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
                    mode: if config::fullscreen() && config::resolution().is_none() {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                    } else {
                        WindowMode::Windowed
                    },
                    position: WindowPosition::Centered(MonitorSelection::Primary),
                    // A window another one covers stops being presented, and a frame that gets no
                    // swapchain texture is a frame that is never drawn, so a sized window stays on
                    // top for the length of the measurement.
                    window_level: if config::resolution().is_some() {
                        WindowLevel::AlwaysOnTop
                    } else {
                        WindowLevel::Normal
                    },
                    resolution: match config::resolution() {
                        Some((width, height)) => {
                            WindowResolution::new(width, height).with_scale_factor_override(1.0)
                        }
                        None => WindowResolution::default(),
                    },
                    present_mode: if config::vsync() {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    },
                    #[cfg(feature = "dev")]
                    desired_maximum_frame_latency: mcrs_minecraft_client::dev::frame_latency(),
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
            .add_before::<sky::SkyPlugin>(
                mcrs_minecraft_client::item_model::resolve::ItemRenderPlugin,
            )
            .add_before::<sky::SkyPlugin>(gui::scene::GuiPlugin)
            .add(screenshot::ScreenshotPlugin),
    )
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .add_systems(
        OnEnter(AppState::Playing),
        (log_registry_counts, log_seeded_resources),
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
    let username = config::username();
    #[cfg(feature = "singleplayer")]
    let server = server_address().unwrap_or_else(|| {
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
        host_integrated_server(world.as_deref(), &assets, traces, host)
    });
    #[cfg(not(feature = "singleplayer"))]
    let server = server_address()
        .expect("a client built without singleplayer hosts no server; set MCRS_SERVER");
    app.add_plugins(ClientNetworkPlugin {
        server,
        username,
        profile_id: save_data.player_uuid,
        view_distance: config::view_distance(),
    });
    app.add_plugins(mcrs_minecraft_client::columns::ColumnCachePlugin);
    app.add_plugins(mcrs_minecraft_client::inventory::InventoryPlugin);
    app.add_plugins(mcrs_minecraft_client::game_mode::GameModePlugin);
    app.add_plugins(gui::game_mode_switcher::GameModeSwitcherPlugin);
    app.insert_resource(ExitOnDisconnect);

    // Inserted after `add_plugins`: `WorldClockPlugin` calls
    // `init_resource::<WorldClocks>()` during its own build, so an earlier
    // insert here would be overwritten.
    let mut world_clocks = WorldClocks::default();
    for (id, mut state) in save_data.world_clocks {
        if let Some(ticks) = frozen_at {
            state.total_ticks = ticks;
            state.partial_tick = 0.0;
        }
        world_clocks.insert(id, state);
    }
    app.insert_resource(world_clocks)
        .insert_resource(AdvanceTime(save_data.advance_time && frozen_at.is_none()))
        .insert_resource(save_data.weather);

    app.add_plugins(ClientTerrainPlugin(terrain_limits(config::view_distance())));
    #[cfg(feature = "dev")]
    app.add_plugins(mcrs_minecraft_client::dev::DevPlugin);

    #[cfg(feature = "telemetry-tracy")]
    app.add_systems(Last, frame_mark);

    let (yaw, pitch) = config::look_override().unwrap_or((save_data.yaw, save_data.pitch));
    if config::look_override().is_some() {
        app.insert_resource(local_player::LookOverride { yaw, pitch });
    }
    let position = config::position_override();
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
    world: Option<&Path>,
    assets: &str,
    traces: Option<ColumnTraceSink>,
    host: SingleplayerProfile,
) -> SocketAddr {
    let open_to_lan = config::open_to_lan();
    let mut server = App::new();
    server.add_plugins(MinecraftServerPlugin {
        bind_address: config::integrated_bind_address(open_to_lan),
        asset_path: Some(assets.to_owned()),
        world: world.map(Path::to_path_buf),
        column_traces: traces,
        singleplayer_profile: Some(host),
        ..MinecraftServerPlugin::embedded()
    });
    let address = server.world().resource::<BoundAddress>().0;
    mcrs_minecraft_server::spawn_server_thread(server, mcrs_minecraft_server::run_server_loop);
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
fn server_address() -> Option<SocketAddr> {
    let address = config::server()?;
    match address.to_socket_addrs().map(|mut a| a.next()) {
        Ok(Some(address)) => Some(address),
        Ok(None) | Err(_) => {
            eprintln!("MCRS_SERVER={address}: expected <host>:<port>");
            std::process::exit(1);
        }
    }
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
        Some(uuid) => {
            match mcrs_minecraft_registry::skip_sets(|| save::read_player_dat(world, uuid)) {
                Ok(Some(player)) => (
                    DVec3::from_array(player.pos),
                    player.rotation[0],
                    player.rotation[1],
                ),
                Ok(None) => spawn_fallback(&level.spawn),
                Err(err) => fatal(err),
            }
        }
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

fn log_registry_counts(registries: Res<RegistrySet>) {
    let loaded = |registry: &str| registries.table(registry).map_or(0, |table| table.len());
    info!(
        dimension_types = loaded("minecraft:dimension_type"),
        biomes = loaded("minecraft:worldgen/biome"),
        timelines = loaded("minecraft:timeline"),
        world_clocks = loaded("minecraft:world_clock"),
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
    advance_time: Res<AdvanceTime>,
    weather: Res<Weather>,
) {
    info!(
        clocks = ?clocks.iter().map(|(id, state)| (id.to_string(), state.total_ticks)).collect::<Vec<_>>(),
        advance_time = advance_time.0,
        rain = weather.rain,
        thunder = weather.thunder,
        "seeded resources from save"
    );
}
