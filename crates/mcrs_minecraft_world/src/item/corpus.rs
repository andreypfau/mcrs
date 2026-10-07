use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_item::Items;

use super::definitions::load_item_definitions;
use crate::registries::test_registries;

/// The whole vanilla corpus, loaded once per process; for tests and tools
/// that have no app to hand it an asset server from.
pub fn test_corpus() -> &'static (Blocks, Items) {
    static CORPUS: OnceLock<(Blocks, Items)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
        ));
        let asset_server = app.world().resource::<AssetServer>().clone();
        let registries = test_registries();
        let block_registry = registries
            .registry::<Block>()
            .expect("the registries report has blocks");
        let (blocks, _) =
            load_block_definitions(&asset_server, &block_registry).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, registries, &blocks)
            .expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}
