use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::asset::io::AssetSourceId;
use bevy::asset::io::file::FileAssetReader;
use bevy::log::{BoxedLayer, LogPlugin};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::WgpuSettings;
use bevy::transform::TransformSystems;
use bevy::window::ExitCondition;
use bevy::window::{
    Monitor, MonitorSelection, PrimaryMonitor, WindowLevel, WindowPosition, WindowResolution,
};
use bevy::winit::WinitPlugin;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::WeatherLevel;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_minecraft_environment::{AdvanceTime, WorldClocks};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;

use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_client::config::{Knobs, MAX_VIEW_DISTANCE, TerrainLimits};
#[cfg(feature = "scene")]
use mcrs_minecraft_client::scene_capture::SceneCapturePlugin;
use mcrs_minecraft_client::screenshot;
use mcrs_minecraft_client::{
    ClientPlugins, ClientTerrainPlugin, config,
    game_directory::{GameDirectory, Platform},
    sky, vanilla, window_options,
};
use mcrs_minecraft_client_gui::menu_plugin::MenuPlugin;
use mcrs_minecraft_client_gui::menu_screens_plugin::MenuScreensPlugin;
use mcrs_minecraft_client_gui::{chat_input, game_mode_switcher};
use mcrs_minecraft_client_world::asset_source::asset_source;
use mcrs_minecraft_client_world::registries::ClientRegistriesPlugin;
#[cfg(feature = "singleplayer")]
use mcrs_minecraft_client_world::world_list::{WorldList, WorldStoreError};
use mcrs_minecraft_client_world::{local_player, player};
#[cfg(feature = "singleplayer")]
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_mesh::SECTION_SIZE;
use mcrs_minecraft_network::client::{ClientNetworkPlugin, JoinRequest};

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

#[cfg(feature = "telemetry-tracy")]
fn frame_mark() {
    tracing_tracy::client::frame_mark();
}

#[cfg(feature = "scene")]
const SCENE_SIZE: UVec2 = UVec2::new(854, 480);

#[cfg(feature = "scene")]
fn scene_dir(knobs: &Knobs) -> Option<PathBuf> {
    knobs.scene_dir()
}

#[cfg(not(feature = "scene"))]
fn scene_dir(_: &Knobs) -> Option<PathBuf> {
    None
}

#[expect(
    clippy::too_many_lines,
    reason = "app setup reads top to bottom in the order added"
)]
pub fn run() -> AppExit {
    let utc_offset = time::UtcOffset::current_local_offset().unwrap_or_else(|error| {
        eprintln!("warning: the time zone of the system: {error}; the world list shows UTC");
        time::UtcOffset::UTC
    });
    #[cfg(target_os = "macos")]
    mcrs_minecraft_client::app_nap::decline();
    let world = world_folder();
    let game_directory = game_directory();
    let knobs = knobs(game_directory.as_ref());
    let frozen_at = knobs.frozen_time();
    let resolution = knobs.resolution();
    let scene = scene_dir(&knobs);
    let view_distance = knobs.view_distance();
    let assets = AssetPlugin::default().file_path;
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
    let defaults = DefaultPlugins
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
            primary_window: scene.is_none().then(|| Window {
                title: match &world {
                    Some(world) => format!("mcrs — {}", world.display()),
                    None => "mcrs".to_owned(),
                },
                mode: window_options::window_mode(knobs.fullscreen()),
                position: WindowPosition::Centered(MonitorSelection::Primary),
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
                present_mode: window_options::present_mode(knobs.vsync()),
                #[cfg(feature = "dev")]
                desired_maximum_frame_latency: knobs.frame_latency(),
                ..default()
            }),
            exit_condition: if scene.is_some() {
                ExitCondition::DontExit
            } else {
                ExitCondition::OnAllClosed
            },
            ..default()
        });
    app.add_plugins(if scene.is_some() {
        defaults
            .disable::<WinitPlugin>()
            .add(ScheduleRunnerPlugin::default())
    } else {
        defaults
    })
    .add_plugins(
        ClientPlugins(knobs.clone())
            .build()
            .set(ClientRegistriesPlugin {
                local_corpus: Some(DirectoryPacks::new(
                    FileAssetReader::get_base_path().join(&assets),
                    data_pack.clone(),
                )),
            })
            .add_before::<sky::SkyPlugin>(mcrs_minecraft_client_gui::slot_icon::SlotIconPlugin)
            .add_before::<sky::SkyPlugin>(knobs.gui())
            .add(screenshot::ScreenshotPlugin),
    )
    .add_systems(
        Update,
        mcrs_minecraft_client::stream::load_gui_atlas.run_if(not(resource_exists::<
            mcrs_minecraft_client_gui::scene::GuiAtlas,
        >)),
    )
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .add_systems(
        Update,
        (log_registry_counts, log_seeded_resources)
            .run_if(resource_exists_and_changed::<CurrentRegistries>),
    )
    .add_systems(
        PostStartup,
        (
            log_spawned_transforms.after(TransformSystems::Propagate),
            log_monitors,
        ),
    );

    #[cfg(feature = "scene")]
    if let Some(dir) = scene {
        app.add_plugins(SceneCapturePlugin {
            dir,
            size: resolution.map_or(SCENE_SIZE, |(width, height)| UVec2::new(width, height)),
            record: knobs.scene_record(),
        });
    }

    let username = knobs.username();
    let local_world = cfg!(feature = "singleplayer") && knobs.server().is_none();
    app.add_plugins(ClientNetworkPlugin {
        holds_vanilla_pack: true,
        play: Some(mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND),
    });
    #[cfg(feature = "singleplayer")]
    {
        #[cfg(feature = "dev")]
        let column_traces = {
            let traces = ColumnTraceSink::default();
            app.insert_resource(traces.clone());
            Some(traces)
        };
        #[cfg(not(feature = "dev"))]
        let column_traces = None;
        #[cfg(feature = "dev")]
        let extend_server = knobs.join_menu_server().unwrap_or(|_| {});
        #[cfg(not(feature = "dev"))]
        let extend_server: fn(&mut App) = |_| {};
        let server = mcrs_minecraft_client::local_server::LocalServerPlugin {
            asset_path: assets.clone(),
            data_pack: data_pack.clone(),
            ops_file: game_directory.as_ref().map(GameDirectory::ops_file),
            username: username.clone(),
            open_to_lan: knobs.open_to_lan(),
            settings: knobs.server_settings(),
            column_traces,
            extend_server,
        };
        if let Some(game_directory) = &game_directory {
            app.add_plugins(mcrs_minecraft_client::world_list::WorldListPlugin {
                saves: game_directory.saves(),
                server: server.clone(),
            });
        } else {
            app.insert_resource(WorldList::Unavailable(WorldStoreError::Io(
                std::io::ErrorKind::NotFound,
            )));
        }
        app.add_plugins(server);
    }
    if let Some(server) = server_address(&knobs) {
        let join = JoinRequest {
            server,
            username,
            profile_id: None,
            view_distance,
        };
        app.add_systems(Startup, move |mut requests: MessageWriter<JoinRequest>| {
            requests.write(join.clone());
        });
    }
    app.add_plugins(mcrs_minecraft_client_world::columns::ColumnCachePlugin::default());
    app.init_resource::<mcrs_minecraft_client_world::screen::Screen>();
    app.add_plugins(mcrs_minecraft_client_entity::ClientEntityPlugin);
    app.add_plugins(game_mode_switcher::GameModeSwitcherPlugin);
    app.add_plugins(mcrs_minecraft_client_gui::inventory_clicks::InventoryClicksPlugin);
    app.add_plugins(mcrs_minecraft_client_world::chat::ClientChatPlugin {
        command_history: knobs
            .command_history()
            .or_else(|| game_directory.as_ref().map(GameDirectory::command_history)),
    });
    app.add_plugins(chat_input::ChatScreenPlugin);
    let preselected = world
        .as_deref()
        .filter(|_| local_world)
        .and_then(|world| preselected_world(world, game_directory.as_ref()));
    app.add_plugins((
        MenuPlugin,
        MenuScreensPlugin {
            preselected_world: preselected.clone(),
            utc_offset,
            singleplayer: cfg!(feature = "singleplayer"),
        },
    ));
    app.insert_resource(AdvanceTime(frozen_at.is_none()))
        .insert_resource(WeatherLevel::default());

    app.add_plugins(ClientTerrainPlugin(terrain_limits(MAX_VIEW_DISTANCE)));
    #[cfg(feature = "dev")]
    {
        use mcrs_minecraft_client::dev::play_world::{self, PlayAtStart};
        app.add_plugins(mcrs_minecraft_client::dev::DevPlugin);
        if let Some(folder_id) = preselected.filter(|_| knobs.play_at_start()) {
            app.insert_resource(PlayAtStart { folder_id })
                .add_systems(Startup, play_world::read_world_list)
                .add_systems(
                    Update,
                    play_world::play_at_start.run_if(resource_exists::<PlayAtStart>),
                );
        }
    }

    #[cfg(feature = "telemetry-tracy")]
    app.add_systems(Last, frame_mark);

    if let Some((yaw, pitch)) = knobs.look_override() {
        app.insert_resource(local_player::LookOverride { yaw, pitch });
    }
    if let Some(position) = knobs.position_override() {
        app.insert_resource(local_player::PositionOverride(position));
    }
    #[cfg(feature = "dev")]
    mcrs_minecraft_client::dev::script_flight(&mut app);

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

fn terrain_limits(view_distance: u8) -> TerrainLimits {
    TerrainLimits {
        arena_scale: 4,
        groups: 1 << 23,
        sections: 1 << 19,
        tint_span: (2 * (u32::from(view_distance) + 2) + 1) * SECTION_SIZE as u32,
    }
}

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

#[expect(
    clippy::disallowed_methods,
    reason = "the client binary reads its settings from the environment"
)]
fn knobs(game_directory: Option<&GameDirectory>) -> Knobs {
    let variables = std::env::vars_os().filter_map(|(name, value)| {
        let name = name.into_string().ok()?;
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
        game_directory.map(GameDirectory::options_file),
    )
}

#[expect(
    clippy::disallowed_methods,
    reason = "the client binary reads its settings from the environment"
)]
fn game_directory() -> Option<GameDirectory> {
    let directory = GameDirectory::pick(Platform::current(), |name| std::env::var_os(name));
    if directory.is_none() {
        eprintln!(
            "no game directory: set MCRS_GAME_DIR; the options and the command history are not kept"
        );
    }
    directory
}

fn preselected_world(world: &Path, game_directory: Option<&GameDirectory>) -> Option<String> {
    let world = std::fs::canonicalize(world).ok()?;
    let saves = game_directory.and_then(|directory| std::fs::canonicalize(directory.saves()).ok());
    if saves.is_none_or(|saves| world.parent() != Some(saves.as_path())) {
        warn!(world = %world.display(), "the world folder is not in the saves folder, no world is preselected");
        return None;
    }
    world
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn world_folder() -> Option<PathBuf> {
    let path = std::env::args_os().nth(1).map(PathBuf::from)?;
    if !path.is_dir() {
        eprintln!("not a world folder: {}", path.display());
        std::process::exit(1);
    }
    Some(path)
}

fn log_registry_counts(registries: Res<CurrentRegistries>) {
    fn count<V: mcrs_minecraft_resource::RegistryValue + Send + Sync + 'static>(
        registries: &CurrentRegistries,
    ) -> usize {
        registries
            .registry::<V>()
            .map_or(0, |registry| registry.len())
    }
    info!(
        dimension_types = count::<DimensionTypeFile>(&registries),
        biomes = count::<BiomeFile>(&registries),
        timelines = count::<mcrs_minecraft_environment::Timeline>(&registries),
        world_clocks = count::<WorldClock>(&registries),
        "registry assets loaded"
    );
}

fn log_spawned_transforms(
    player: Single<(&Transform, &PhysicsTransform), With<player::Player>>,
    camera: Single<&GlobalTransform, With<mcrs_minecraft_client::camera::PlayerCamera>>,
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
    weather: Res<WeatherLevel>,
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
