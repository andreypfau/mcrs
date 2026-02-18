use crate::tag::{block_tags, item_tags};
use crate::world::block::Block;
use crate::world::item::Item;
use bevy_app::{App, Plugin, PostStartup, Startup, Update};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_core::tag::file::TagEntry;
use mcrs_core::tag::file::TagFile;
use mcrs_core::tag::key::TagRegistryType;
use mcrs_core::{AppState, StaticId, StaticRegistry, StaticTags};
use mcrs_registry::Registry;
use std::collections::HashSet;
use std::str::FromStr;

pub struct VanillaPlugin;

impl Plugin for VanillaPlugin {
    fn build(&self, app: &mut App) {
        app
            // New registry resources (parallel to old Registry<T>)
            .init_resource::<StaticRegistry<Block>>()
            .init_resource::<StaticRegistry<Item>>()
            // New tag resources (parallel to old TagRegistry<T>)
            .init_resource::<StaticTags<Block>>()
            .init_resource::<StaticTags<Item>>()
            // Populate static registries at Startup (after MinecraftBlockPlugin
            // and MinecraftItemPlugin have inserted their Registry<T> resources)
            .add_systems(Startup, (populate_block_registry, populate_item_registry))
            // Bootstrap → LoadingDataPack after all Startup systems
            .add_systems(PostStartup, start_loading_data_pack)
            // Demand-load all tag files when entering LoadingDataPack
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (request_block_tags, request_item_tags),
            )
            // Poll for completion every frame
            .add_systems(
                Update,
                check_tags_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            // Resolve tags and transition to Playing
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (resolve_block_tags, resolve_item_tags, transition_to_playing).chain(),
            );
    }
}

fn populate_block_registry(
    old: Res<Registry<&'static Block>>,
    mut new: ResMut<StaticRegistry<Block>>,
) {
    for (ident, block) in old.iter_entries() {
        let loc = ResourceLocation::from_str(&ident.to_string())
            .expect("block identifier must be a valid resource location");
        new.register(loc, *block);
    }
    tracing::debug!(count = new.len(), "populated StaticRegistry<Block>");
}

fn populate_item_registry(
    old: Res<Registry<&'static Item>>,
    mut new: ResMut<StaticRegistry<Item>>,
) {
    for (ident, item) in old.iter_entries() {
        let loc = ResourceLocation::from_str(&ident.to_string())
            .expect("item identifier must be a valid resource location");
        new.register(loc, *item);
    }
    tracing::debug!(count = new.len(), "populated StaticRegistry<Item>");
}

fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

fn request_block_tags(mut tags: ResMut<StaticTags<Block>>, asset_server: Res<AssetServer>) {
    tags.request(&block_tags::MINEABLE_PICKAXE, &asset_server);
    tags.request(&block_tags::MINEABLE_AXE, &asset_server);
    tags.request(&block_tags::MINEABLE_SHOVEL, &asset_server);
    tags.request(&block_tags::MINEABLE_HOE, &asset_server);
    tags.request(&block_tags::NEEDS_CORRECT_TOOL, &asset_server);
    tags.request(&block_tags::LOGS, &asset_server);
    tags.request(&block_tags::LEAVES, &asset_server);
    tags.request(&block_tags::SAND, &asset_server);
    tags.request(&block_tags::WOOL, &asset_server);
    tags.request(&block_tags::SNOW, &asset_server);
}

fn request_item_tags(mut tags: ResMut<StaticTags<Item>>, asset_server: Res<AssetServer>) {
    tags.request(&item_tags::SWORDS, &asset_server);
    tags.request(&item_tags::PICKAXES, &asset_server);
    tags.request(&item_tags::AXES, &asset_server);
    tags.request(&item_tags::SHOVELS, &asset_server);
    tags.request(&item_tags::HOES, &asset_server);
}

fn check_tags_ready(
    block_tags: Res<StaticTags<Block>>,
    item_tags: Res<StaticTags<Item>>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server) && item_tags.all_handles_loaded(&asset_server) {
        tracing::info!("all static tag files loaded — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}

fn resolve_block_tags(
    mut tags: ResMut<StaticTags<Block>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<Block>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = expand_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("block tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(resolved_entries = resolved, "resolved StaticTags<Block>");
}

fn resolve_item_tags(
    mut tags: ResMut<StaticTags<Item>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<Item>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = expand_tag_file(tf, &tag_files, &registry);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("item tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(resolved_entries = resolved, "resolved StaticTags<Item>");
}

fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}

/// Recursively expand a `TagFile` into a set of `StaticId<T>`.
///
/// Nested `#tag` references are resolved via `all` (the full `Assets<TagFile>` store).
/// Required entries that cannot be found in the registry emit a warning; optional
/// entries are silently skipped.
fn expand_tag_file<T: TagRegistryType + 'static>(
    tag_file: &TagFile,
    all: &Assets<TagFile>,
    registry: &StaticRegistry<T>,
) -> HashSet<StaticId<T>> {
    let mut out = HashSet::new();
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) => {
                if let Some(id) = registry.id_of(loc) {
                    out.insert(id);
                } else {
                    tracing::warn!("tag references unknown registry entry: {loc}");
                }
            }
            TagEntry::OptionalElement(loc) => {
                if let Some(id) = registry.id_of(loc) {
                    out.insert(id);
                }
            }
            TagEntry::Tag(h) | TagEntry::OptionalTag(h) => {
                if let Some(nested) = all.get(h) {
                    out.extend(expand_tag_file(nested, all, registry));
                }
            }
        }
    }
    out
}
