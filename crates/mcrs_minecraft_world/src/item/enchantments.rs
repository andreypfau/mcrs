use std::path::Path;
use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_tasks::block_on;

use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::enchantment::data::ProtoEnchantmentData;
use mcrs_minecraft_registry::{NameTable, StaticRegistry};

use crate::registries::test_registries;

pub fn register_all_enchantments(
    registry: &mut StaticRegistry<EnchantmentData>,
    table: &NameTable,
    asset_server: &AssetServer,
) {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let reader = source.reader();
    for loc in table.names() {
        let path = format!("{}/enchantment/{}.json", loc.namespace(), loc.path());
        let bytes = block_on(read_whole(reader, Path::new(&path)))
            .unwrap_or_else(|e| panic!("failed to read enchantment file {path}: {e}"));
        let proto: ProtoEnchantmentData = serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("failed to parse enchantment JSON {path}: {e}"));
        let data = proto
            .resolve()
            .unwrap_or_else(|e| panic!("failed to resolve enchantment {path}: {e}"));
        let leaked: &'static EnchantmentData = Box::leak(Box::new(data));
        registry.register(loc.clone(), leaked);
    }
}

/// The vanilla enchantments, loaded once per process; for tests that have no
/// app to hand them an asset server from.
pub fn test_enchantments() -> &'static StaticRegistry<EnchantmentData> {
    static REGISTRY: OnceLock<StaticRegistry<EnchantmentData>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
        ));
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut registry = StaticRegistry::new();
        let table = test_registries()
            .table("minecraft:enchantment")
            .expect("minecraft:enchantment is a loaded registry");
        register_all_enchantments(&mut registry, table, &asset_server);
        registry.freeze();
        registry
    })
}
