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
use mcrs_minecraft_world::save::{self, SaveError};
use mcrs_minecraft_world::timeline::Timeline;
use mcrs_minecraft_world::world_clock::{AdvanceTime, WorldClock, WorldClocks};
use mcrs_voxel_world::entity::physics::Transform as PhysicsTransform;

use mcrs_minecraft_client::render::{
    FACE_BYTES, Layout, MODEL_BYTES, QUAD_BYTES, TerrainPlugin, Uploads,
};
use mcrs_minecraft_client::{
    anvil, asset_corpus, camera, cave, config, gui, input, local_player, pack, player, render,
    screenshot, sky, stream,
};

fn main() {
    let world = world_folder();
    let save_data = load_save(&world);
    let frozen_at = frozen_time();
    let terrain = terrain_source(&world, &save_data.dimension, save_data.position);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_corpus().to_string_lossy().into_owned(),
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
                    title: format!("mcrs — {}", world.display()),
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

    match terrain {
        Ok((layout, uploads, cave, loader)) => {
            app.add_plugins(TerrainPlugin(layout, uploads))
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
        }
        Err(error) => error!("no terrain to draw: {error}"),
    }

    let (yaw, pitch) = look_override().unwrap_or((save_data.yaw, save_data.pitch));
    player::spawn_player(app.world_mut(), save_data.position, yaw, pitch);

    app.run();
}

fn region_folder(world: &Path, dimension: &str) -> PathBuf {
    let (namespace, path) = dimension.split_once(':').unwrap_or(("minecraft", dimension));
    world
        .join("dimensions")
        .join(namespace)
        .join(path)
        .join("region")
}

const BUDGET_FILES: usize = 4;
const GROUPS_PER_FILE: usize = 1 << 18;

/// The region files around the player, until columns arrive from the network.
fn terrain_source(
    world: &Path,
    dimension: &str,
    position: DVec3,
) -> Result<(Arc<Layout>, Uploads, cave::CaveCull, stream::Loader), String> {
    let centre = config::window_centre().unwrap_or_else(|| {
        let region = |axis: f64| (axis / anvil::REGION_BLOCKS as f64).floor() as i32;
        [region(position.x), region(position.z)]
    });
    let window = anvil::window(&region_folder(world, dimension), centre, config::region_window())?;

    let chunks = anvil::REGION_CHUNKS;
    let extent = [
        window.regions[0] * chunks,
        anvil::SECTIONS_Y,
        window.regions[1] * chunks,
    ];
    let files = window.files.len().clamp(1, BUDGET_FILES);
    let (quad_mb, model_mb, face_mb) = config::arena_budget();
    let span = anvil::REGION_BLOCKS as u32;
    let layout = Arc::new(Layout {
        grid: pack::RegionGrid::covering(extent),
        min_section: [
            window.min_region[0] * chunks as i32,
            anvil::MIN_SECTION_Y,
            window.min_region[1] * chunks as i32,
        ],
        quad_capacity: quad_mb * files * 1_000_000 / QUAD_BYTES,
        model_capacity: model_mb * files * 1_000_000 / MODEL_BYTES,
        face_capacity: face_mb * files * 1_000_000 / FACE_BYTES,
        group_capacity: GROUPS_PER_FILE * files,
        cave_words: (cave::cave_grid().slots() + pack::SECTIONS_PER_RENDER_REGION).div_ceil(32),
        tint_origin: [
            window.min_region[0] * span as i32,
            window.min_region[1] * span as i32,
        ],
        tint_size: [
            window.regions[0] as u32 * span,
            window.regions[1] as u32 * span,
        ],
    });

    let cave = cave::CaveCull::new(cave::cave_grid(), layout.min_section, extent);
    assert_eq!(
        cave.words(),
        layout.cave_words,
        "the sight-line bitset and the buffer it goes into have to be the same size"
    );
    info!(
        files = window.files.len(),
        min_region = ?window.min_region,
        regions = ?window.regions,
        draws = layout.max_draws(),
        quad_mb = (layout.quad_capacity * QUAD_BYTES) / 1_000_000,
        model_mb = (layout.model_capacity * MODEL_BYTES) / 1_000_000,
        face_mb = (layout.face_capacity * FACE_BYTES) / 1_000_000,
        "streaming terrain from region files"
    );

    let uploads = Uploads::default();
    let loader = stream::Loader::new(layout.clone(), uploads.clone(), window);
    Ok((layout, uploads, cave, loader))
}

fn world_folder() -> PathBuf {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: mcrs_minecraft_client <world folder>");
        std::process::exit(1);
    };
    if !path.is_dir() {
        eprintln!("not a world folder: {}", path.display());
        std::process::exit(1);
    }
    path
}

struct SaveData {
    world_clocks: save::WorldClockStates,
    dimension: String,
    advance_time: bool,
    weather: Weather,
    position: DVec3,
    yaw: f32,
    pitch: f32,
}

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
