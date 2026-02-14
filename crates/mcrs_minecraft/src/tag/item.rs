use crate::tag::block::{load_tags_from_directory, process_loaded_tags, TagRegistry};
use crate::tag::loader::{ResourcePackTags, ResourcePackTagsLoader};
use crate::world::item::Item;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetServer};
use bevy_ecs::system::ResMut;

pub struct ItemTagPlugin;

impl Plugin for ItemTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ResourcePackTags>()
            .register_asset_loader(ResourcePackTagsLoader)
            .init_resource::<TagRegistry<&'static Item>>()
            .add_systems(Startup, load_item_tags)
            .add_systems(Update, process_loaded_tags::<&'static Item>);
    }
}

fn load_item_tags(
    asset_server: ResMut<AssetServer>,
    mut registry: ResMut<TagRegistry<&'static Item>>,
) {
    load_tags_from_directory(&asset_server, &mut registry, "minecraft/tags/item");
}
