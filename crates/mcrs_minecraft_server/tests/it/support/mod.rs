#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_item::{Items, load_item_definitions};

/// The real corpus, read once per test binary. A dimension sub-app is handed
/// this at spawn, and worldgen resolves the block it fills terrain with against
/// it, so a stub would only move the failure somewhere less obvious.
pub fn standalone_corpus() -> &'static (Blocks, Items) {
    static CORPUS: OnceLock<(Blocks, Items)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let (blocks, _) = load_block_definitions(&asset_server).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, &blocks).expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}

pub fn insert_corpus(app: &mut App) {
    let (blocks, items) = standalone_corpus();
    app.insert_resource(blocks.clone());
    app.insert_resource(items.clone());
}
