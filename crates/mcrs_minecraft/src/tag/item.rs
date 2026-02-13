use crate::tag::block::{process_loaded_tags, TagRegistry};
use crate::tag::loader::{
    ResourcePackTags, ResourcePackTagsLoader, TagFileLoaderSettings,
};
use crate::world::item::Item;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetServer, Handle};
use bevy_ecs::system::ResMut;
use std::fs;
use std::path::PathBuf;

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
    // Recursively load all item tag files from assets/minecraft/tags/item/
    let base_path = PathBuf::from("assets/minecraft/tags/item");

    if !base_path.exists() {
        tracing::warn!("Item tags directory not found: {:?}", base_path);
        return;
    }

    if let Ok(tag_files) = collect_tag_files(&base_path, &base_path) {
        tracing::info!("Loading {} item tag files", tag_files.len());

        for relative_path in tag_files {
            let asset_path = format!("minecraft/tags/item/{}", relative_path);
            let handle: Handle<ResourcePackTags> = asset_server.load_with_settings(
                asset_path.clone(),
                |settings: &mut TagFileLoaderSettings| {
                    settings.directory = "minecraft/tags/item".to_string();
                },
            );
            registry.loaded_tags.push(handle);
        }
    }
}

/// Recursively collects all .json files in the directory
fn collect_tag_files(dir: &PathBuf, base_path: &PathBuf) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recursively collect from subdirectories
            files.extend(collect_tag_files(&path, base_path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            // Get relative path from base_path
            if let Ok(relative) = path.strip_prefix(base_path) {
                if let Some(relative_str) = relative.to_str() {
                    files.push(relative_str.to_string());
                }
            }
        }
    }

    Ok(files)
}
