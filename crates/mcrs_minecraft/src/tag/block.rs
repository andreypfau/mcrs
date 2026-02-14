use crate::tag::loader::{
    ResourcePackTags, ResourcePackTagsLoader, TagEntry, TagFileLoaderSettings, TagOrTagFileHandle,
};
use crate::world::block::Block;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_asset::{AssetApp, AssetEvent, AssetId, AssetServer, Assets, Handle};
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::{Res, ResMut};
use bevy_ecs_macros::Resource;
use mcrs_protocol::packets::configuration::clientbound::{RegistryTags, TagGroup};
use mcrs_protocol::{Ident, VarInt};
use mcrs_registry::{Registry, RegistryId};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    Tag(&'static Block),
    TagSet(BlockTagSet),
}

/// A dynamic block tag set that wraps an identifier for runtime lookup.
///
/// Unlike the static `BlockTagSet`, this type stores only the tag identifier
/// and performs lookups against the `TagRegistry` at runtime. This enables
/// tags loaded from asset files to be used in place of hardcoded constants.
///
/// # Example
///
/// ```ignore
/// let pickaxe_tag = DynamicBlockTagSet::new(ident!("minecraft:mineable/pickaxe"));
///
/// // Later, check if a block is in the tag
/// if pickaxe_tag.contains_block(&block_registry, &tag_registry, &block) {
///     // Block can be mined with a pickaxe
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DynamicBlockTagSet {
    /// The identifier of the tag (e.g., "minecraft:mineable/pickaxe")
    pub ident: Ident<String>,
}

impl DynamicBlockTagSet {
    /// Creates a new dynamic block tag set from the given identifier.
    pub fn new(ident: Ident<String>) -> Self {
        Self { ident }
    }

    /// Creates a new dynamic block tag set from a static identifier.
    pub fn from_static(ident: Ident<&'static str>) -> Self {
        Self {
            ident: ident.to_string_ident(),
        }
    }

    /// Returns the identifier of this tag set.
    pub fn ident(&self) -> &Ident<String> {
        &self.ident
    }

    /// Returns the registry IDs of all blocks in this tag, or `None` if the tag doesn't exist.
    pub fn get_entries<'a>(
        &self,
        tag_registry: &'a TagRegistry<&'static Block>,
    ) -> Option<&'a [RegistryId<&'static Block>]> {
        tag_registry.get_tag(&self.ident)
    }

    /// Checks if the given block is contained in this tag set.
    ///
    /// Returns `false` if the tag doesn't exist or if the block is not in the tag.
    pub fn contains_block(
        &self,
        tag_registry: &TagRegistry<&'static Block>,
        block: &RegistryId<&'static Block>,
    ) -> bool {
        tag_registry.contains_tag(&self.ident, block)
    }

    /// Checks if the given block (by registry index) is contained in this tag set.
    ///
    /// Returns `false` if the tag doesn't exist or if the block is not in the tag.
    pub fn contains_block_index(
        &self,
        tag_registry: &TagRegistry<&'static Block>,
        block_index: usize,
    ) -> bool {
        let block_id = RegistryId::<&'static Block>::Index {
            index: block_index,
            marker: std::marker::PhantomData,
        };
        self.contains_block(tag_registry, &block_id)
    }
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

pub struct BlockTagPlugin;

impl Plugin for BlockTagPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ResourcePackTags>()
            .register_asset_loader(ResourcePackTagsLoader)
            .init_resource::<TagRegistry<&'static Block>>()
            // .add_systems(Startup, load_block_tags)
            // .add_systems(Update, process_loaded_tags::<&'static Block>)
        ;
    }
}

#[derive(Resource)]
pub struct TagRegistry<T: Clone + Send + Sync> {
    pub map: HashMap<Ident<String>, Vec<RegistryId<T>>>,
    pub(crate) loaded_tags: Vec<Handle<ResourcePackTags>>,
    processed_tags: HashSet<AssetId<ResourcePackTags>>,
}

impl<T: std::clone::Clone + Send + std::marker::Sync> Default for TagRegistry<T> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            loaded_tags: Vec::new(),
            processed_tags: HashSet::new(),
        }
    }
}

impl<T: Clone + Send + Sync> TagRegistry<T> {
    /// Returns the registry IDs for all entries in the specified tag, or None if the tag doesn't exist.
    pub fn get_tag(&self, tag_name: &Ident<String>) -> Option<&[RegistryId<T>]> {
        self.map.get(tag_name).map(|v| v.as_slice())
    }

    /// Checks if the specified registry ID is contained in the given tag.
    ///
    /// Returns `false` if the tag doesn't exist or if the entry is not in the tag.
    pub fn contains_tag(&self, tag_name: &Ident<String>, entry: &RegistryId<T>) -> bool
    where
        RegistryId<T>: PartialEq,
    {
        self.get_tag(tag_name)
            .map(|entries| entries.contains(entry))
            .unwrap_or(false)
    }

    /// Returns an iterator over all tag names in the registry.
    pub fn iter_tags(&self) -> impl Iterator<Item = &Ident<String>> {
        self.map.keys()
    }

    /// Converts the tag registry data into a `RegistryTags` structure suitable for
    /// the `ClientboundUpdateTags` packet.
    ///
    /// # Arguments
    ///
    /// * `registry_name` - The identifier of the registry (e.g., "minecraft:block", "minecraft:item")
    ///
    /// # Returns
    ///
    /// A `RegistryTags` structure containing all tags with their numeric entry IDs.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let block_tags: TagRegistry<&'static Block> = ...;
    /// let registry_tags = block_tags.build_registry_tags(ident!("minecraft:block"));
    /// ```
    pub fn build_registry_tags<'a>(&self, registry_name: Ident<Cow<'a, str>>) -> RegistryTags<'a> {
        let tags: Vec<TagGroup<'a>> = self
            .map
            .iter()
            .map(|(tag_name, entries)| {
                let entry_ids: Vec<VarInt> = entries
                    .iter()
                    .filter_map(|id| match id {
                        RegistryId::Index { index, .. } => Some(VarInt(*index as i32)),
                        _ => None,
                    })
                    .collect();

                TagGroup {
                    name: Ident::new(Cow::Owned(tag_name.to_string())).unwrap_or_else(|_| {
                        Ident::new(Cow::Borrowed("minecraft:unknown")).unwrap()
                    }),
                    entries: entry_ids,
                }
            })
            .collect();

        RegistryTags {
            registry: registry_name,
            tags,
        }
    }

    fn resolve_tag_entries<'a, 'b: 'a>(
        &mut self,
        tag_name: Ident<String>,
        entries: &'b [TagEntry],
        tags_assets: &Assets<ResourcePackTags>,
        registry: &'a Registry<T>,
    ) {
        let mut blocks = Vec::new();

        // info!("Resolving tag entries for {}", tag_name);

        for entry in entries {
            match &entry.tag {
                TagOrTagFileHandle::Tag(ident) => {
                    let reg_id = RegistryId::Identifier {
                        identifier: ident.clone(),
                    };
                    if let Some((index, block)) = registry.get_full(reg_id) {
                        blocks.push(RegistryId::Index {
                            index,
                            marker: std::marker::PhantomData,
                        });
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

fn load_block_tags(
    asset_server: ResMut<AssetServer>,
    mut registry: ResMut<TagRegistry<&'static Block>>,
) {
    load_tags_from_directory(&asset_server, &mut registry, "minecraft/tags/block");
}

/// Loads tag files from a directory into a `TagRegistry`.
///
/// The `tag_directory` should be relative to the `assets/` folder,
/// e.g. `"minecraft/tags/block"` or `"minecraft/tags/item"`.
pub(crate) fn load_tags_from_directory<T: Clone + Send + Sync + 'static>(
    asset_server: &AssetServer,
    registry: &mut TagRegistry<T>,
    tag_directory: &str,
) {
    let base_path = PathBuf::from(format!("assets/{}", tag_directory));

    if !base_path.exists() {
        tracing::warn!("Tags directory not found: {:?}", base_path);
        return;
    }

    match collect_tag_files(&base_path, &base_path) {
        Ok(tag_files) => {
            tracing::info!("Loading {} tag files from {}", tag_files.len(), tag_directory);

            for relative_path in tag_files {
                let asset_path = format!("{}/{}", tag_directory, relative_path);
                let dir = tag_directory.to_string();
                let handle: Handle<ResourcePackTags> = asset_server.load_with_settings(
                    asset_path,
                    move |settings: &mut TagFileLoaderSettings| {
                        settings.directory = dir.clone();
                    },
                );
                registry.loaded_tags.push(handle);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read tag files from {:?}: {}", base_path, e);
        }
    }
}

/// Recursively collects all .json files in the directory.
fn collect_tag_files(dir: &PathBuf, base_path: &PathBuf) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(collect_tag_files(&path, base_path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(relative) = path.strip_prefix(base_path) {
                if let Some(relative_str) = relative.to_str() {
                    files.push(relative_str.to_string());
                }
            }
        }
    }

    Ok(files)
}

pub fn process_loaded_tags<T: Clone + Send + Sync + 'static>(
    mut registry: ResMut<TagRegistry<T>>,
    tags_assets: Res<Assets<ResourcePackTags>>,
    block_registry: Res<Registry<T>>,
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
                if let Some(tag_name) = asset_path_to_tag_ident(&path_str) {
                    registry.resolve_tag_entries(
                        tag_name,
                        &tag_asset.values,
                        &tags_assets,
                        &block_registry,
                    );
                    registry.processed_tags.insert(asset_id);
                } else {
                    tracing::warn!("Could not parse tag asset path '{}' into a valid tag identifier", path_str);
                }
            }
        }
    }
}

/// Converts a tag asset path to a Minecraft tag identifier.
///
/// Transforms paths like:
/// - `"minecraft/tags/block/mineable/pickaxe.json"` -> `"minecraft:mineable/pickaxe"`
/// - `"minecraft/tags/item/logs.json"` -> `"minecraft:logs"`
///
/// Returns `None` if the path doesn't match the expected `namespace/tags/type/path.json` format.
fn asset_path_to_tag_ident(path: &str) -> Option<Ident<String>> {
    let stripped = path.strip_suffix(".json")?;

    let tags_idx = stripped.find("/tags/")?;
    let namespace = &stripped[..tags_idx];
    let after_tags = &stripped[tags_idx + 6..]; // skip "/tags/"

    let type_sep_idx = after_tags.find('/')?;
    let tag_path = &after_tags[type_sep_idx + 1..];

    let ident_str = format!("{}:{}", namespace, tag_path);
    Ident::<String>::from_str(&ident_str).ok()
}
