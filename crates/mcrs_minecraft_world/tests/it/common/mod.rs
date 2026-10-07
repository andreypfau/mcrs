use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::{Registered, RegistrySet};
use mcrs_minecraft_world::MinecraftWorldPlugin;
use mcrs_minecraft_world::item::test_corpus;
use serde::Deserialize;

pub fn items() -> &'static Items {
    &test_corpus().1
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

pub fn declared_registries(stable: bool) -> BTreeSet<String> {
    datapack_report()
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements && flags.stable == stable)
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

pub fn tag_members<R: Registered>(set: &RegistrySet, tag: &str) -> Vec<String> {
    let registry = set.registry::<R>().expect("the registry is loaded");
    let tags = set
        .tags::<R>()
        .unwrap_or_else(|| panic!("the load builds the tags of {}", R::REGISTRY));
    let key = TagKey::<R, _>::from_location(ResourceLocation::read(tag).unwrap());
    let id = tags
        .get(&key)
        .unwrap_or_else(|| panic!("{tag} is a loaded tag of {}", R::REGISTRY));
    tags.members(id)
        .map(|member| registry.name(member).unwrap().as_str().to_owned())
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
        mcrs_minecraft_assets::packs::layered_file_source(
            "assets",
            mcrs_minecraft_worldgen_builtin::asset,
        ),
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
