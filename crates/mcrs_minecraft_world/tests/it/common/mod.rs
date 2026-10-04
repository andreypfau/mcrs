use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::MinecraftWorldPlugin;
use mcrs_minecraft_world::item::test_corpus;
use serde::Deserialize;

pub fn corpus() -> &'static (Blocks, Items) {
    test_corpus()
}

pub fn items() -> &'static Items {
    &corpus().1
}

pub fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

pub fn assets() -> PathBuf {
    workspace_root().join("assets")
}

#[derive(Deserialize)]
pub struct Flags {
    pub elements: bool,
    pub stable: bool,
}

#[derive(Deserialize)]
pub struct DatapackReport {
    pub registries: BTreeMap<String, Flags>,
}

pub fn datapack_report() -> DatapackReport {
    let path = assets().join("mcrs/reports/datapack.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn declared_world_registries() -> BTreeSet<String> {
    datapack_report()
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements && !flags.stable)
        .map(|(registry, _)| registry)
        .collect()
}

pub fn loaded_names(set: &RegistrySet, registry: &str) -> Vec<String> {
    set.table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"))
        .names()
        .iter()
        .map(ToString::to_string)
        .collect()
}

pub fn run_to_playing() -> App {
    std::env::set_current_dir(workspace_root()).unwrap();
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_test_writer()
        .try_init();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(StatesPlugin);
    bevy_asset::AssetApp::register_asset_source(
        &mut app,
        bevy_asset::io::AssetSourceId::Default,
        mcrs_minecraft_worldgen::bevy::asset_source("assets"),
    );
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
    app.finish();
    app.cleanup();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        app.update();
        if *app.world().resource::<State<AppState>>().get() == AppState::Playing {
            return app;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never reached Playing"
        );
    }
}
