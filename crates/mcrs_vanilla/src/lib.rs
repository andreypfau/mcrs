#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::too_many_arguments
)]

pub mod block;
pub mod item;
pub mod material;
pub mod sound;

use crate::block::tags as block_tags;
use crate::item::tags as item_tags;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_core::tag::file::{TagEntry, TagFile};
use mcrs_core::tag::key::TagRegistryType;
use mcrs_core::{AppState, ResourceLocation, StaticId, StaticRegistry, StaticTags};
use std::collections::HashSet;

pub struct MinecraftCorePlugin;

impl Plugin for MinecraftCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StaticRegistry<block::Block>>()
            .init_resource::<StaticRegistry<item::Item>>()
            .init_resource::<StaticTags<block::Block>>()
            .init_resource::<StaticTags<item::Item>>()
            .add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                (request_block_tags, request_item_tags),
            )
            .add_systems(
                Update,
                check_tags_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (resolve_block_tags, resolve_item_tags, transition_to_playing).chain(),
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

fn request_item_tags(mut tags: ResMut<StaticTags<item::Item>>, asset_server: Res<AssetServer>) {
    tags.request(&item_tags::SWORDS, &asset_server);
    tags.request(&item_tags::PICKAXES, &asset_server);
    tags.request(&item_tags::AXES, &asset_server);
    tags.request(&item_tags::SHOVELS, &asset_server);
    tags.request(&item_tags::HOES, &asset_server);
}

fn check_tags_ready(
    block_tags: Res<StaticTags<block::Block>>,
    item_tags: Res<StaticTags<item::Item>>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if block_tags.all_handles_loaded(&asset_server) && item_tags.all_handles_loaded(&asset_server) {
        tracing::info!("all static tag files loaded — entering WorldgenFreeze");
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
