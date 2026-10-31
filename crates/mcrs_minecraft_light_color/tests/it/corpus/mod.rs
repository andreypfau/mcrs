use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_assets::asset::read_json_corpus;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::Items;

pub fn blocks() -> &'static Blocks {
    &mcrs_minecraft_engine_registries::test_corpus().0
}

pub fn items() -> &'static Items {
    &mcrs_minecraft_engine_registries::test_corpus().1
}

fn asset_server() -> &'static AssetServer {
    static SERVER: OnceLock<AssetServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
        ));
        app.world().resource::<AssetServer>().clone()
    })
}

pub fn corpus(directory: &str) -> Vec<(String, Vec<u8>)> {
    let source = asset_server()
        .get_source(AssetSourceId::Default)
        .expect("the default asset source");
    read_json_corpus(source.reader(), directory).unwrap_or_else(|e| panic!("{e}"))
}
