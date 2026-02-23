#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod block;
pub mod enchantment;
pub mod entity;
pub mod explosion;
pub mod item;
pub mod material;
pub mod player_action;
pub mod sound;

use crate::block::tags as block_tags;
use crate::enchantment::data::EnchantmentData;
use crate::enchantment::registry::{LoadedEnchantments, VANILLA_ENCHANTMENTS};
use crate::enchantment::tags as enchantment_tags;
use crate::enchantment::EnchantmentDataLoader;
use crate::item::tags as item_tags;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetApp, AssetId, AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::tag::file::{TagEntry, TagFile};
use mcrs_core::tag::key::TagRegistryType;
use mcrs_core::{AppState, ResourceLocation, StaticId, StaticRegistry, StaticTags, Tags};
use std::collections::HashSet;

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StaticRegistry<block::Block>>()
            .init_resource::<StaticRegistry<item::Item>>()
            .init_resource::<StaticTags<block::Block>>()
            .init_resource::<StaticTags<item::Item>>()
            .init_asset::<EnchantmentData>()
            .register_asset_loader(EnchantmentDataLoader)
            .init_resource::<Tags<EnchantmentData>>()
            .add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (
                    request_block_tags,
                    request_item_tags,
                    request_enchantment_assets,
                    request_enchantment_tags,
                ),
            )
            .add_systems(
                Update,
                check_tags_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (
                    resolve_block_tags,
                    resolve_item_tags,
                    resolve_enchantment_tags,
                    freeze_static_tags,
                    transition_to_playing,
                )
                    .chain(),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let mut blocks = app
                .world_mut()
                .resource_mut::<StaticRegistry<block::Block>>();
            block::minecraft::register_all_blocks(&mut blocks);
            tracing::info!(count = blocks.len(), "registered StaticRegistry<Block>");
        }
        {
            let mut items = app.world_mut().resource_mut::<StaticRegistry<item::Item>>();
            item::minecraft::register_all_items(&mut items);
            tracing::info!(count = items.len(), "registered StaticRegistry<Item>");
        }
    }
}

fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

fn request_block_tags(mut tags: ResMut<StaticTags<block::Block>>, asset_server: Res<AssetServer>) {
    for tag in block_tags::ALL_BLOCK_TAGS {
        tags.request(tag, &asset_server);
    }
}

fn request_item_tags(mut tags: ResMut<StaticTags<item::Item>>, asset_server: Res<AssetServer>) {
    for tag in item_tags::ALL_ITEM_TAGS {
        tags.request(tag, &asset_server);
    }
}

fn request_enchantment_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut loaded = LoadedEnchantments::new();
    for &name in VANILLA_ENCHANTMENTS {
        let loc = ResourceLocation::parse(name).expect("invalid enchantment RL");
        let path = format!("{}/enchantment/{}.json", loc.namespace(), loc.path());
        let handle = asset_server.load::<EnchantmentData>(path);
        loaded.push(loc, handle);
    }
    tracing::info!(count = loaded.len(), "requested enchantment assets");
    commands.insert_resource(loaded);
}

fn request_enchantment_tags(
    mut tags: ResMut<Tags<EnchantmentData>>,
    asset_server: Res<AssetServer>,
) {
    for tag_key in enchantment_tags::ALL_ENCHANTMENT_TAGS {
        tags.request(tag_key, &asset_server);
    }
    tracing::info!(
        count = enchantment_tags::ALL_ENCHANTMENT_TAGS.len(),
        "requested enchantment tag files"
    );
}

fn check_tags_ready(
    block_tags: Res<StaticTags<block::Block>>,
    item_tags: Res<StaticTags<item::Item>>,
    enchantment_tags: Res<Tags<EnchantmentData>>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server)
        && item_tags.all_handles_loaded(&asset_server)
        && enchantment_tags.all_handles_loaded(&asset_server)
    {
        tracing::info!("all tag files loaded — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}

fn resolve_block_tags(
    mut tags: ResMut<StaticTags<block::Block>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<block::Block>>,
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
    mut tags: ResMut<StaticTags<item::Item>>,
    tag_files: Res<Assets<TagFile>>,
    registry: Res<StaticRegistry<item::Item>>,
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

fn resolve_enchantment_tags(
    mut tags: ResMut<Tags<EnchantmentData>>,
    tag_files: Res<Assets<TagFile>>,
    loaded: Res<LoadedEnchantments>,
    enchantment_assets: Res<Assets<EnchantmentData>>,
) {
    let handles = tags.drain_handles();
    let mut resolved = 0usize;
    for (loc, handle) in handles {
        if let Some(tf) = tag_files.get(&handle) {
            let ids = expand_dynamic_tag_file(tf, &tag_files, &loaded, &enchantment_assets);
            resolved += ids.len();
            tags.insert(loc, ids);
        } else {
            tracing::warn!("enchantment tag file not available at WorldgenFreeze: {loc}");
        }
    }
    tracing::info!(
        resolved_entries = resolved,
        "resolved Tags<EnchantmentData>"
    );
}

fn freeze_static_tags(
    mut block_tags: ResMut<StaticTags<block::Block>>,
    mut item_tags: ResMut<StaticTags<item::Item>>,
    block_registry: Res<StaticRegistry<block::Block>>,
    item_registry: Res<StaticRegistry<item::Item>>,
) {
    block_tags.freeze(block_registry.len() as u32);
    item_tags.freeze(item_registry.len() as u32);
    tracing::info!("frozen StaticTags<Block> and StaticTags<Item>");
}

fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}

/// Recursively expand a `TagFile` into a set of `StaticId<T>`.
fn expand_tag_file<T: TagRegistryType + 'static>(
    tag_file: &TagFile,
    all: &Assets<TagFile>,
    registry: &StaticRegistry<T>,
) -> HashSet<StaticId<T>> {
    let mut out = HashSet::new();
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) => {
                if let Some(id) = registry.id_of(loc.as_str()) {
                    out.insert(id);
                } else {
                    tracing::warn!("tag references unknown registry entry: {loc}");
                }
            }
            TagEntry::OptionalElement(loc) => {
                if let Some(id) = registry.id_of(loc.as_str()) {
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

/// Recursively expand a `TagFile` into a set of `AssetId<EnchantmentData>`,
/// resolving element references via `LoadedEnchantments`.
fn expand_dynamic_tag_file(
    tag_file: &TagFile,
    all: &Assets<TagFile>,
    loaded: &LoadedEnchantments,
    assets: &Assets<EnchantmentData>,
) -> HashSet<AssetId<EnchantmentData>> {
    let mut out = HashSet::new();
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) => {
                if let Some(id) = loaded.resolve_asset_id(loc, assets) {
                    out.insert(id);
                } else {
                    tracing::warn!("enchantment tag references unknown entry: {loc}");
                }
            }
            TagEntry::OptionalElement(loc) => {
                if let Some(id) = loaded.resolve_asset_id(loc, assets) {
                    out.insert(id);
                }
            }
            TagEntry::Tag(h) | TagEntry::OptionalTag(h) => {
                if let Some(nested) = all.get(h) {
                    out.extend(expand_dynamic_tag_file(nested, all, loaded, assets));
                }
            }
        }
    }
    out
}
