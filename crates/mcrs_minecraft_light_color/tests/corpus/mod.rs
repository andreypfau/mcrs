#![allow(dead_code)]

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_assets::tag::file::SerializedTagFile;
use mcrs_minecraft_assets::tag::registry::TagSource;
use mcrs_minecraft_assets::tag::{DynTagRegistry, TagLoader};
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};

pub fn blocks() -> &'static Blocks {
    &mcrs_minecraft_item::definition::test_corpus().0
}

pub fn items() -> &'static Items {
    &mcrs_minecraft_item::definition::test_corpus().1
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

/// Every block tag of every namespace, expanded off the files themselves.
pub fn block_tags() -> &'static DynTagRegistry<Block> {
    static TAGS: OnceLock<DynTagRegistry<Block>> = OnceLock::new();
    TAGS.get_or_init(|| {
        let blocks = blocks();
        let mut loader = TagLoader::<Block, u32>::default();
        for namespace in std::fs::read_dir(assets_dir()).unwrap() {
            let namespace = namespace.unwrap().path();
            let dir = namespace.join("tags/block");
            if !dir.is_dir() {
                continue;
            }
            let namespace = namespace
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            for path in json_files(&dir) {
                let name = path.strip_prefix(&dir).unwrap().with_extension("");
                let name = format!("{namespace}:{}", name.to_string_lossy().replace('\\', "/"));
                let mut members = HashSet::new();
                collect(blocks, &name, &mut members);
                loader.insert(ResourceLocation::read(&name).unwrap(), members);
            }
        }
        loader.freeze(blocks)
    })
}

fn collect(blocks: &Blocks, name: &str, into: &mut HashSet<u32>) {
    let location = ResourceLocation::read(name).unwrap();
    let path = assets_dir()
        .join(location.namespace())
        .join("tags/block")
        .join(format!("{}.json", location.path()));
    let file: SerializedTagFile = read(&path);
    for entry in file.values {
        if entry.id.is_tag {
            collect(blocks, entry.id.loc.as_str(), into);
        } else if let Some(index) = blocks.id_of(entry.id.loc.as_str()) {
            into.insert(index);
        }
    }
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
