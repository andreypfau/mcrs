use std::time::{Duration, Instant};

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::asset::io::AssetSourceId;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use mcrs_minecraft_assets::packs::layered_file_source;
use mcrs_minecraft_assets::{AppState, MinecraftCorePlugin};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_client::asset_corpus;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironments;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::MinecraftWorldPlugin;

const BOOT_DEADLINE: Duration = Duration::from_secs(120);

pub fn boot(extra: impl FnOnce(&mut App)) -> App {
    let assets = asset_corpus().to_string_lossy().into_owned();
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: TaskPoolOptions::with_num_threads(2),
    })
    .add_plugins(StatesPlugin)
    .register_asset_source(
        AssetSourceId::Default,
        layered_file_source(&assets, mcrs_minecraft_worldgen_builtin::asset),
    );
    app.add_plugins(AssetPlugin {
        file_path: assets,
        watch_for_changes_override: Some(false),
        ..default()
    })
    .add_plugins((MinecraftCorePlugin, MinecraftWorldPlugin));
    extra(&mut app);
    app.finish();
    app.cleanup();

    let deadline = Instant::now() + BOOT_DEADLINE;
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }
    app
}

#[test]
fn the_client_boots_to_playing_with_its_local_registries() {
    let started = Instant::now();
    let app = boot(|_| {});
    println!("booted to Playing in {:?}", started.elapsed());

    let registries = app.world().resource::<RegistrySet>();
    let biomes = registries
        .registry::<Biome>()
        .expect("the biome registry is loaded");
    let loaded_biomes = registries
        .entries::<Biome, Biome>()
        .expect("the biome column is loaded");
    assert!(!biomes.is_empty());
    assert_eq!(loaded_biomes.as_slice().len(), biomes.len());

    let types = registries
        .registry::<DimensionType>()
        .expect("the dimension type registry is loaded");
    let loaded_types = registries
        .entries::<DimensionType, DimensionType>()
        .expect("the dimension type column is loaded");
    assert!(!types.is_empty());
    assert_eq!(loaded_types.as_slice().len(), types.len());

    let environments = app.world().resource::<DimensionEnvironments>();
    assert_eq!(
        environments.len(),
        types.len(),
        "every dimension type has the environment its sky is built from"
    );
}
