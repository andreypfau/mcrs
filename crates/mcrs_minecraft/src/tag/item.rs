use crate::tag::block::{load_tags_from_directory, process_loaded_tags, TagRegistry};
use crate::tag::loader::{ResourcePackTags, ResourcePackTagsLoader};
use crate::world::item::Item;
use crate::world::item::minecraft;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetServer};
use bevy_ecs::system::ResMut;
use mcrs_registry::Registry;

pub struct ItemTagPlugin;

impl Plugin for ItemTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ResourcePackTags>()
            .register_asset_loader(ResourcePackTagsLoader)
            .init_resource::<TagRegistry<&'static Item>>()
            .insert_resource(init_item_registry())
            .add_systems(Startup, load_item_tags)
            .add_systems(Update, process_loaded_tags::<&'static Item>)
        ;
    }
}

fn init_item_registry() -> Registry<&'static Item> {
    let mut registry = Registry::new();
    registry.insert(minecraft::WOODEN_PICKAXE.identifier, &minecraft::WOODEN_PICKAXE);
    registry.insert(minecraft::STONE_PICKAXE.identifier, &minecraft::STONE_PICKAXE);
    registry.insert(minecraft::GOLDEN_PICKAXE.identifier, &minecraft::GOLDEN_PICKAXE);
    registry.insert(minecraft::IRON_PICKAXE.identifier, &minecraft::IRON_PICKAXE);
    registry.insert(minecraft::DIAMOND_PICKAXE.identifier, &minecraft::DIAMOND_PICKAXE);
    registry
}

fn load_item_tags(
    asset_server: ResMut<AssetServer>,
    mut registry: ResMut<TagRegistry<&'static Item>>,
) {
    load_tags_from_directory(&asset_server, &mut registry, "minecraft/tags/item");
}
