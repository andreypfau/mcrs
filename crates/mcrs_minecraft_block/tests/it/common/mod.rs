use std::sync::LazyLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{BlockDefinitions, load_block_definitions};
use mcrs_minecraft_registry::static_report::shipped_report;

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
            .registry_of(mcrs_minecraft_block::keys::BLOCK)
            .expect("the registries report has blocks");
        load_block_definitions(&asset_server, &blocks)
            .expect("the corpus loads")
            .0
    });
    &CORPUS
}

pub fn assert_no_mismatches(what: &str, mismatches: Vec<String>) {
    assert!(
        mismatches.is_empty(),
        "{} {what}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
