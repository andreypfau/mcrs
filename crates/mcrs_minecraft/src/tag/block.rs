use crate::tag::loader::{
    ResourcePackTags, ResourcePackTagsLoader, TagEntry, TagFileLoaderSettings, TagOrTagFileHandle,
};
use crate::world;
use crate::world::block::Block;
use bevy_app::{App, FixedUpdate, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetEvent, AssetId, AssetServer, Assets, Handle};
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs_macros::Resource;
use mcrs_protocol::{Ident, ident};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tracing::info;
use world::block::minecraft;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    Tag(&'static Block),
    TagSet(BlockTagSet),
}

pub trait BlockTagSetExt {
    fn contains_block(&self, block: &Block) -> bool;
}

impl BlockTagSetExt for BlockTag {
    fn contains_block(&self, block: &Block) -> bool {
        match self {
            BlockTag::Tag(b) => b == &block,
            BlockTag::TagSet(tag_set) => tag_set.contains_block(block),
        }
    }
}

impl BlockTagSetExt for BlockTagSet {
    fn contains_block(&self, block: &Block) -> bool {
        for tag in *self {
            if tag.contains_block(block) {
                return true;
            }
        }
        false
    }
}

pub const MINEABLE_PICKAXE: BlockTagSet = &[&BlockTag::Tag(&minecraft::STONE)];

pub const MINEABLE_SHOVEL: BlockTagSet = &[
    &BlockTag::Tag(&minecraft::GRASS_BLOCK),
    &BlockTag::Tag(&minecraft::DIRT),
];

pub const NEEDS_DIAMOND_TOOL: BlockTagSet = &[];
pub const NEEDS_IRON_TOOL: BlockTagSet = &[];
pub const NEEDS_STONE_TOOL: BlockTagSet = &[];

pub const INCORRECT_FOR_NETHERITE_TOOL: BlockTagSet = &[];
pub const INCORRECT_FOR_DIAMOND_TOOL: BlockTagSet = &[];
pub const INCORRECT_FOR_IRON_TOOL: BlockTagSet = &[&BlockTag::TagSet(NEEDS_DIAMOND_TOOL)];
pub const INCORRECT_FOR_COPPER_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
];
pub const INCORRECT_FOR_STONE_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
];
pub const INCORRECT_FOR_GOLD_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
    &BlockTag::TagSet(NEEDS_STONE_TOOL),
];
pub const INCORRECT_FOR_WOODEN_TOOL: BlockTagSet = &[
    &BlockTag::TagSet(NEEDS_DIAMOND_TOOL),
    &BlockTag::TagSet(NEEDS_IRON_TOOL),
    &BlockTag::TagSet(NEEDS_STONE_TOOL),
];

pub struct BlockTagPlugin;

impl Plugin for BlockTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ResourcePackTags>()
            .register_asset_loader(ResourcePackTagsLoader)
            .init_resource::<BlockTagRegistry>()
            .add_systems(Startup, load_block_tags)
            .add_systems(FixedUpdate, process_loaded_tags);
    }
}

#[derive(Resource, Default)]
pub struct BlockTagRegistry {
    pub map: HashMap<Ident<String>, Vec<&'static Block>>,
    loaded_tags: Vec<Handle<ResourcePackTags>>,
    processed_tags: HashSet<AssetId<ResourcePackTags>>,
}

impl BlockTagRegistry {
    fn resolve_tag_entries(
        &mut self,
        tag_name: Ident<String>,
        entries: &[TagEntry],
        tags_assets: &Assets<ResourcePackTags>,
    ) {
        let mut blocks = Vec::new();

        info!("Resolving tag entries for {}", tag_name);

        for entry in entries {
            match &entry.tag {
                TagOrTagFileHandle::Tag(ident) => {
                    // Try to convert identifier to Block reference
                    let result: Result<&'static Block, ()> = TryFrom::try_from(ident.clone());
                    if let Ok(block) = result {
                        info!("Found block {} in tag {}", block.identifier, tag_name);
                        blocks.push(block);
                    } else if !entry.required {
                        // If not required and not found, skip silently
                        continue;
                    }
                }
                TagOrTagFileHandle::TagFile(handle) => {
                    // Recursively load referenced tag file
                    if let Some(tag_file) = tags_assets.get(handle) {
                        if let Some(path) = handle.path() {
                            let path_str = path.path().to_string_lossy().into_owned();
                            if let Ok(tag_key) = Ident::<String>::from_str(&path_str) {
                                if let Some(referenced_blocks) = self.map.get(&tag_key) {
                                    blocks.extend_from_slice(referenced_blocks);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.map.insert(tag_name, blocks);
    }
}

fn load_block_tags(mut asset_server: ResMut<AssetServer>, mut registry: ResMut<BlockTagRegistry>) {
    // Load all block tag files from assets/minecraft/tags/block/
    let tag_files = vec![
        "mineable/pickaxe.json",
        "mineable/axe.json",
        "mineable/shovel.json",
        "mineable/hoe.json",
        "needs_diamond_tool.json",
        "needs_iron_tool.json",
        "needs_stone_tool.json",
        "incorrect_for_netherite_tool.json",
        "incorrect_for_diamond_tool.json",
        "incorrect_for_iron_tool.json",
        "incorrect_for_copper_tool.json",
        "incorrect_for_stone_tool.json",
        "incorrect_for_gold_tool.json",
        "incorrect_for_wooden_tool.json",
    ];

    for tag_file in tag_files {
        let path = format!("minecraft/tags/block/{}", tag_file);
        let handle: Handle<ResourcePackTags> = asset_server.load_with_settings(
            path.clone(),
            |settings: &mut TagFileLoaderSettings| {
                settings.directory = "minecraft/tags/block".to_string();
            },
        );
        registry.loaded_tags.push(handle);
    }
}

pub fn process_loaded_tags(
    mut registry: ResMut<BlockTagRegistry>,
    tags_assets: Res<Assets<ResourcePackTags>>,
    mut asset_events: MessageReader<AssetEvent<ResourcePackTags>>,
) {
    // Handle asset events for hot reload support
    let mut changed_tags = false;
    asset_events.read().for_each(|event| {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                // Mark as unprocessed so it will be reprocessed
                registry.processed_tags.remove(&id);
                changed_tags = true;
            }
            AssetEvent::Removed { id } => {
                registry.processed_tags.remove(&id);
                changed_tags = true;
            }
            _ => {}
        }
    });
    if !changed_tags {
        return;
    }

    let handles = registry.loaded_tags.clone();
    for handle in &handles {
        let asset_id = handle.id();

        // Skip if already processed
        if registry.processed_tags.contains(&asset_id) {
            continue;
        }

        if let Some(tag_asset) = tags_assets.get(handle) {
            if let Some(path) = handle.path() {
                let path_str = path.path().to_string_lossy().into_owned();
                if let Ok(tag_name) = Ident::<String>::from_str(&path_str) {
                    registry.resolve_tag_entries(tag_name, &tag_asset.values, &tags_assets);
                    registry.processed_tags.insert(asset_id);
                }
            }
        }
    }
}
