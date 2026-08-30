#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::AssetPlugin;
use bevy::camera::visibility::VisibilitySystems;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::WgpuSettings;
use bevy::transform::TransformSystems;
use bevy::winit::{UpdateMode, WinitSettings};
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::dimension::dimension_type::DimensionType;
use mcrs_minecraft_world::environment::Weather;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_world::save::{self, SaveError};
use mcrs_minecraft_world::timeline::Timeline;
use mcrs_minecraft_world::world_clock::{AdvanceTime, WorldClock, WorldClocks};
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

use mcrs_minecraft_client::render::{
    Budget, FACE_BYTES, MODEL_BYTES, QUAD_BYTES, TerrainPlugin, Uploads, VISIBLE_BYTES,
};
use mcrs_minecraft_client::sky_state::SkyEffects;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugin};
use mcrs_minecraft_client::{
    asset_corpus, camera, cave, config, gui, input, local_player, player, render, sky, sky_render,
    stream,
};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_client::screenshot;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_network::client::ClientNetworkPlugin;

/// The browser has no world folder and no save to seed itself from, so the
/// entry point there is its own.
#[cfg(target_family = "wasm")]
fn main() {
    mcrs_minecraft_client::web::run();
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    let world = world_folder();
    let save_data = world.as_deref().map(load_save).unwrap_or_default();
    let frozen_at = frozen_time();
    let (budget, uploads, cave, loader) = terrain(save_data.position);
    let assets = asset_corpus().to_string_lossy().into_owned();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: assets.clone(),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    features: WgpuFeatures::TIMESTAMP_QUERY,
                    ..default()
                }
                .into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: match &world {
                        Some(world) => format!("mcrs — {}", world.display()),
                        None => "mcrs".to_owned(),
                    },
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    })
    .add_plugins(mcrs_minecraft_core::MinecraftCorePlugin)
    .add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin)
    .add_plugins(player::PlayerPlugin)
    .add_plugins(input::ClientInputPlugin)
    .add_plugins(local_player::LocalPlayerPlugin)
    .add_plugins(camera::CameraPlugin)
    .add_plugins(gui::debug::DebugScreenPlugin)
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .add_plugins(sky::SkyPlugin)
    .add_plugins(screenshot::ScreenshotPlugin)
    .add_systems(
        OnEnter(AppState::Playing),
        (log_registry_counts, log_seeded_resources),
    )
    .add_systems(
        PostStartup,
        log_spawned_transforms.after(TransformSystems::Propagate),
    );

    if let Some(only) = sky_draws_only() {
        app.insert_resource(sky_render::SkyDrawsOnly(only));
    }

    // After `DefaultPlugins`: an embedded server leaves the task pools to its
    // host, so the host has to have built them before the server thread ticks.
    let server =
        server_address().unwrap_or_else(|| host_integrated_server(world.as_deref(), &assets));
    app.add_plugins(ClientNetworkPlugin {
        server,
        username: std::env::var("MCRS_USERNAME").unwrap_or_else(|_| "Player".to_owned()),
    });

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
        .insert_resource(save_data.weather)
        .insert_resource(sky::PlayerDimension(save_data.dimension));

    app.add_plugins(TerrainPlugin(budget, uploads))
        .insert_resource(config::drawn_streams())
        .insert_resource(config::raster_fraction())
        .insert_resource(cave)
        .insert_resource(loader)
        .add_systems(
            Update,
            (
                stream::advance,
                cave::toggle,
                render::toggle_wireframe,
                #[cfg(target_os = "macos")]
                mcrs_minecraft_client::capture::gputrace,
            ),
        )
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        );

    let (yaw, pitch) = look_override().unwrap_or((save_data.yaw, save_data.pitch));
    player::spawn_player(app.world_mut(), save_data.position, yaw, pitch);

    app.run();
}

/// One block holds every group of every bucket and a flush takes a fresh one before freeing the
/// stale one, so the arena has to fit two of them with the buddy rounding on top.
const GROUPS_BUDGET: usize = 1 << 21;

const SECTIONS_BUDGET: usize = 1 << 16;

/// The tint texture covers a fixed square of world around the spawn.
/// ponytail: a player who walks out of it takes the edge tint with them; the
/// upgrade is a tint window that scrolls with the camera.
const TINT_SPAN: u32 = 1024;

const ARENA_SCALE: usize = 4;

fn terrain(spawn: DVec3) -> (Arc<Budget>, Uploads, cave::CaveCull, stream::Loader) {
    let (quad_mb, model_mb, face_mb) = config::arena_budget();
    let centre = |axis: f64| (axis as i32).div_euclid(16) * 16 - TINT_SPAN as i32 / 2;
    let budget = Arc::new(Budget {
        quads: quad_mb * ARENA_SCALE * 1_000_000 / QUAD_BYTES,
        models: model_mb * ARENA_SCALE * 1_000_000 / MODEL_BYTES,
        faces: face_mb * ARENA_SCALE * 1_000_000 / FACE_BYTES,
        groups: GROUPS_BUDGET,
        sections: SECTIONS_BUDGET,
        visible: config::visible_budget() / VISIBLE_BYTES,
        tint_origin: [centre(spawn.x), centre(spawn.z)],
        tint_size: [TINT_SPAN; 2],
    });

    info!(
        quad_mb = (budget.quads * QUAD_BYTES) / 1_000_000,
        model_mb = (budget.models * MODEL_BYTES) / 1_000_000,
        face_mb = (budget.faces * FACE_BYTES) / 1_000_000,
        visible_mb = (budget.visible * VISIBLE_BYTES) / 1_000_000,
        "meshing the columns the server sends"
    );

    let uploads = Uploads::default();
    let loader = stream::Loader::new(&budget, uploads.clone());
    (
        budget.clone(),
        uploads,
        cave::CaveCull::new(budget.sections),
        loader,
    )
}

/// Singleplayer, the way the vanilla client plays it: a server of our own on a
/// loopback port, which the client then joins like any other.
#[cfg(not(target_family = "wasm"))]
fn host_integrated_server(world: Option<&Path>, assets: &str) -> SocketAddr {
    let mut server = App::new();
    server.add_plugins(MinecraftServerPlugin::embedded().with_assets(assets));
    let address = server.world().resource::<BoundAddress>().0;
    mcrs_minecraft_server::spawn_server_thread(server, mcrs_minecraft_server::run_server_loop);
    match world {
        Some(world) => info!(
            world = %world.display(),
            %address,
            "hosting an integrated server, which generates its own terrain rather than \
             reading the saved chunks of this world",
        ),
        None => info!(%address, "hosting an integrated server"),
    }
    address
}

/// `MCRS_SERVER=<host>:<port>` joins that server instead of hosting one.
fn server_address() -> Option<SocketAddr> {
    let address = std::env::var("MCRS_SERVER").ok()?;
    match address.to_socket_addrs().map(|mut a| a.next()) {
        Ok(Some(address)) => Some(address),
        Ok(None) | Err(_) => {
            eprintln!("MCRS_SERVER={address}: expected <host>:<port>");
            std::process::exit(1);
        }
    }
}

/// A world folder seeds the clocks, the weather and the spawn. Terrain comes
/// from the server either way, so there need not be one.
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
    dimension: String,
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
            dimension: "minecraft:overworld".to_owned(),
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

    let (position, yaw, pitch, dimension) = match level.singleplayer_uuid {
        Some(uuid) => match save::read_player(world, uuid) {
            Ok(player) => (
                DVec3::from_array(player.pos),
                player.yaw,
                player.pitch,
                player.dimension,
            ),
            Err(SaveError::Missing { .. }) => spawn_fallback(&level.spawn),
            Err(err) => fatal(err),
        },
        None => spawn_fallback(&level.spawn),
    };

    SaveData {
        world_clocks,
        dimension,
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
fn spawn_fallback(spawn: &save::RespawnData) -> (DVec3, f32, f32, String) {
    (
        DVec3::new(
            spawn.pos[0] as f64 + 0.5,
            spawn.pos[1] as f64,
            spawn.pos[2] as f64 + 0.5,
        ),
        spawn.yaw,
        spawn.pitch,
        "minecraft:overworld".to_owned(),
    )
}

/// `MCRS_LOOK=<yaw>,<pitch>` aims the camera somewhere other than where the
/// save left it, in Minecraft degrees.
fn look_override() -> Option<(f32, f32)> {
    let look = std::env::var("MCRS_LOOK").ok()?;
    let angles = look
        .split_once(',')
        .and_then(|(yaw, pitch)| Some((yaw.trim().parse().ok()?, pitch.trim().parse().ok()?)));
    let Some(angles) = angles else {
        eprintln!("MCRS_LOOK={look}: expected <yaw>,<pitch> in degrees");
        std::process::exit(1);
    };
    Some(angles)
}

/// `MCRS_SKY=disc,twilight,celestial,stars,clouds` draws only the passes it
/// lists, which is how a frame gets priced one pass at a time.
fn sky_draws_only() -> Option<SkyEffects> {
    let list = std::env::var("MCRS_SKY").ok()?;
    match SkyEffects::parse(&list) {
        Ok(effects) => Some(effects),
        Err(err) => {
            eprintln!("MCRS_SKY={list}: {err}");
            std::process::exit(1);
        }
    }
}

/// `MCRS_TIME=<ticks>` pins every clock and stops them, so a scripted
/// screenshot lands on the tick it asked for.
fn frozen_time() -> Option<i64> {
    let ticks = std::env::var("MCRS_TIME").ok()?;
    match ticks.trim().parse() {
        Ok(ticks) => Some(ticks),
        Err(err) => {
            eprintln!("MCRS_TIME={ticks}: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
fn fatal(err: SaveError) -> ! {
    eprintln!("{err}");
    std::process::exit(1);
}

fn log_registry_counts(
    dimension_types: Res<Assets<DimensionType>>,
    biomes: Res<Assets<Biome>>,
    timelines: Res<Assets<Timeline>>,
    world_clocks: Res<Assets<WorldClock>>,
) {
    info!(
        dimension_types = dimension_types.len(),
        biomes = biomes.len(),
        timelines = timelines.len(),
        world_clocks = world_clocks.len(),
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
