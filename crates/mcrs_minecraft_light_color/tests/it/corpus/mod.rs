#![allow(dead_code)]

use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registry::{RegistrySet, Tags};
use mcrs_minecraft_world::registries::test_registries;

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

pub fn registries() -> &'static RegistrySet {
    test_registries()
}

pub fn block_tags() -> &'static Tags<Block> {
    static TAGS: OnceLock<Tags<Block>> = OnceLock::new();
    TAGS.get_or_init(|| {
        registries()
            .tags::<Block>()
            .expect("the load builds the block tags")
    })
}
