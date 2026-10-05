use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{
    BlockDefinitions, CORPUS_DIRECTORY, load_block_definitions,
};
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_registry::static_report::shipped_report;
use serde::de::DeserializeOwned;

pub fn corpus() -> &'static BlockDefinitions {
    static CORPUS: LazyLock<BlockDefinitions> = LazyLock::new(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let blocks = shipped_report()
            .registry::<Block>()
            .expect("the registries report has blocks");
        load_block_definitions(&asset_server, &blocks)
            .expect("the corpus loads")
            .0
    });
    &CORPUS
}

pub fn definition_files<T: DeserializeOwned>() -> Vec<(PathBuf, T)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(CORPUS_DIRECTORY);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|e| panic!("{}: {e}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let file = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            (path, file)
        })
        .collect()
}

pub fn assert_no_mismatches(what: &str, mismatches: Vec<String>) {
    assert!(
        mismatches.is_empty(),
        "{} {what}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
