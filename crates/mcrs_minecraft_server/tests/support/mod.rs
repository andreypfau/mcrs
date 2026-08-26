#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_world::block::definition::{BlockDefinitions, Blocks, load_block_definitions};

/// The real corpus, read once per test binary. A dimension sub-app is handed
/// this at spawn, and worldgen resolves the block it fills terrain with against
/// it, so a stub would only move the failure somewhere less obvious.
pub fn corpus(app: &App) -> Blocks {
    static CORPUS: OnceLock<Blocks> = OnceLock::new();
    CORPUS
        .get_or_init(|| {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let (definitions, _) =
                load_block_definitions(&asset_server).expect("the block definition corpus loads");
            Blocks(Arc::new(definitions))
        })
        .clone()
}

/// The corpus for a test that builds no `App` of its own.
pub fn standalone_corpus() -> &'static BlockDefinitions {
    static CORPUS: OnceLock<BlockDefinitions> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        load_block_definitions(&asset_server)
            .expect("the block definition corpus loads")
            .0
    })
}
