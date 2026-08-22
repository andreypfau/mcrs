use std::path::{Path, PathBuf};

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use mcrs_core::AppState;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::dimension::dimension_type::DimensionType;
use mcrs_vanilla::timeline::Timeline;
use mcrs_vanilla::world_clock::WorldClock;

fn main() {
    let world = world_folder();

    App::new()
        .add_plugins(
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
        .add_systems(OnEnter(AppState::Playing), log_registry_counts)
        .run();
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
