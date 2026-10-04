#![allow(dead_code)]

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_assets::tag::file::SerializedTagFile;
use mcrs_minecraft_assets::tag::{DynTagRegistry, TagLoader};
use mcrs_minecraft_block::definition::{Blocks, Fluids};
use mcrs_minecraft_block::{Block, Fluid};
use mcrs_minecraft_core::{ResourceLocation, TaggedRegistry};
use mcrs_minecraft_item::{Item, Items};
use mcrs_minecraft_registry::TagSource;
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

pub fn block_tags() -> &'static DynTagRegistry<Block> {
    static TAGS: OnceLock<DynTagRegistry<Block>> = OnceLock::new();
    TAGS.get_or_init(|| every_tag(blocks()))
}

pub fn item_tags() -> &'static DynTagRegistry<Item> {
    static TAGS: OnceLock<DynTagRegistry<Item>> = OnceLock::new();
    TAGS.get_or_init(|| every_tag(items()))
}

pub fn fluid_tags() -> &'static DynTagRegistry<Fluid> {
    static TAGS: OnceLock<DynTagRegistry<Fluid>> = OnceLock::new();
    TAGS.get_or_init(|| every_tag(&Fluids(blocks().0.clone())))
}

/// Every tag of one registry in every namespace, expanded off the files
/// themselves.
pub fn every_tag<T: TaggedRegistry, S: TagSource<Id = u32>>(source: &S) -> DynTagRegistry<T> {
    let mut loader = TagLoader::<T, u32>::default();
    for namespace in std::fs::read_dir(assets_dir()).unwrap() {
        let namespace = namespace.unwrap().path();
        let dir = namespace.join("tags").join(T::REGISTRY_PATH);
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
            collect(T::REGISTRY_PATH, source, &name, &mut members);
            loader.insert(ResourceLocation::read(&name).unwrap(), members);
        }
    }
    loader.freeze(source)
}

fn collect<S: TagSource<Id = u32>>(
    registry: &str,
    source: &S,
    name: &str,
    into: &mut HashSet<u32>,
) {
    let location = ResourceLocation::read(name).unwrap();
    let path = assets_dir()
        .join(location.namespace())
        .join("tags")
        .join(registry)
        .join(format!("{}.json", location.path()));
    let file: SerializedTagFile = read(&path);
    for entry in file.values {
        if entry.id.is_tag {
            collect(registry, source, entry.id.loc.as_str(), into);
        } else if let Some(index) = source.id_of(entry.id.loc.as_str()) {
            into.insert(index);
        }
    }
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
