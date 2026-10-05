use std::path::Path;
use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_item::{Item, Items};
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_registry::static_report::from_report;

use self::definitions::load_item_definitions;

pub mod definitions;
pub mod enchantments;
pub mod tool;

pub use enchantments::{test_enchantment_registry, test_enchantments};

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
        let source = asset_server
            .get_source(AssetSourceId::Default)
            .expect("default AssetSource missing");
        let report = bevy_tasks::block_on(read_whole(
            source.reader(),
            Path::new("mcrs/reports/registries.json"),
        ))
        .expect("the registries report reads");
        let registries = from_report(&report).expect("the registries report parses");
        let block_registry = registries
            .registry::<Block>()
            .expect("the registries report has blocks");
        let item_registry = registries
            .registry::<Item>()
            .expect("the registries report has items");
        let (blocks, _) =
            load_block_definitions(&asset_server, &block_registry).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, &item_registry, &blocks)
            .expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}
