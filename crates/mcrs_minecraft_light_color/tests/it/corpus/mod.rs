#![allow(dead_code)]

use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::Items;

pub fn blocks() -> &'static Blocks {
    &mcrs_minecraft_world::item::test_corpus().0
}

pub fn items() -> &'static Items {
    &mcrs_minecraft_world::item::test_corpus().1
}

pub fn asset_server() -> &'static AssetServer {
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
