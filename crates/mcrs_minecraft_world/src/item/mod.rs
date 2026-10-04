use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::StaticRegistry;

use self::definitions::load_item_definitions;

pub mod definitions;

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
        let (blocks, _) = load_block_definitions(&asset_server).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, &blocks).expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}

pub fn test_enchantments() -> &'static StaticRegistry<EnchantmentData> {
    mcrs_minecraft_item::enchantment::test_enchantments()
}
