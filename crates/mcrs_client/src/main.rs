use std::path::{Path, PathBuf};

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use mcrs_core::AppState;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::dimension::dimension_type::DimensionType;
use mcrs_vanilla::environment::Weather;
use mcrs_vanilla::save::{self, SaveError};
use mcrs_vanilla::timeline::Timeline;
use mcrs_vanilla::world_clock::{AdvanceTime, WorldClock, WorldClocks};

mod player;
mod screenshot;
mod sky;
mod sky_render;

fn main() {
    let world = world_folder();
    let save_data = load_save(&world);
    let frozen_at = frozen_time();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_corpus().to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("mcrs — {}", world.display()),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(mcrs_core::MinecraftEnginePlugin)
    .add_plugins(mcrs_vanilla::MinecraftCorePlugin)
    .add_plugins(player::PlayerPlugin)
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

    let (yaw, pitch) = look_override().unwrap_or((save_data.yaw, save_data.pitch));
    player::spawn_player(app.world_mut(), save_data.translation, yaw, pitch);

    app.run();
}

fn world_folder() -> PathBuf {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: mcrs_client <world folder>");
        std::process::exit(1);
    };
    if !path.is_dir() {
        eprintln!("not a world folder: {}", path.display());
        std::process::exit(1);
    }
    path
}

fn asset_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets")
}

struct SaveData {
    world_clocks: save::WorldClockStates,
    dimension: String,
    advance_time: bool,
    weather: Weather,
    translation: Vec3,
    yaw: f32,
    pitch: f32,
}

fn load_save(world: &Path) -> SaveData {
    let level = save::read_level_dat(world).unwrap_or_else(|err| fatal(err));
    let world_clocks = save::read_world_clocks(world).unwrap_or_else(|err| fatal(err));
    let weather = save::read_weather(world).unwrap_or_else(|err| fatal(err));
    let game_rules = save::read_game_rules(world).unwrap_or_else(|err| fatal(err));

    let (translation, yaw, pitch, dimension) = match level.singleplayer_uuid {
        Some(uuid) => match save::read_player(world, uuid) {
            Ok(player) => (
                Vec3::new(player.pos[0] as f32, player.pos[1] as f32, player.pos[2] as f32),
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
        translation,
        yaw,
        pitch,
    }
}

/// A spawn point is a block position; the player stands at its centre in X and
/// Z, and Y is the block's own floor.
fn spawn_fallback(spawn: &save::RespawnData) -> (Vec3, f32, f32, String) {
    (
        Vec3::new(spawn.pos[0] as f32 + 0.5, spawn.pos[1] as f32, spawn.pos[2] as f32 + 0.5),
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
    player: Single<(&Transform, &player::PlayerLook), With<player::Player>>,
    camera: Single<&GlobalTransform, With<player::PlayerCamera>>,
) {
    let (transform, look) = player.into_inner();
    info!(
        player_translation = ?transform.translation,
        camera_world_translation = ?camera.translation(),
        yaw = look.yaw,
        pitch = look.pitch,
        "spawned player"
    );
}

fn log_seeded_resources(clocks: Res<WorldClocks>, advance_time: Res<AdvanceTime>, weather: Res<Weather>) {
    info!(
        clocks = ?clocks.iter().map(|(id, state)| (id.to_string(), state.total_ticks)).collect::<Vec<_>>(),
        advance_time = advance_time.0,
        rain = weather.rain,
        thunder = weather.thunder,
        "seeded resources from save"
    );
}
