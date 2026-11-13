#![expect(
    clippy::expect_used,
    reason = "test support reports a failure by panicking"
)]

use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_item::Items;

use crate::item_definitions::load_items;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_table::ItemTable;

/// The whole vanilla corpus, loaded once per process; for tests and tools
/// that have no app to hand it an asset server from.
pub fn test_corpus() -> &'static (Blocks, Items) {
    &corpus().0
}

/// The item table of the whole vanilla corpus, baked with [`test_corpus`].
pub fn test_item_table() -> ItemTable {
    corpus().1
}

fn corpus() -> &'static ((Blocks, Items), ItemTable) {
    static CORPUS: OnceLock<((Blocks, Items), ItemTable)> = OnceLock::new();
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
        let (blocks, _) = load_block_definitions(&asset_server).expect("the block corpus loads");
        let items = load_items(&asset_server, registries, &blocks).expect("the item corpus loads");
        (
            (Blocks(Arc::new(blocks)), Items(Arc::new(items.definitions))),
            items.table,
        )
    })
}
