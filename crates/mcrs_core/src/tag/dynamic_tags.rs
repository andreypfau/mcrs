use crate::resource_location::ResourceLocation;
use crate::tag::file::{TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TagRegistryType};
use bevy_asset::{Asset, AssetId, AssetServer, Handle};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Resolved tags for dynamic asset types (biomes, dimension types, etc.).
///
/// Populated during `OnEnter(AppState::WorldgenFreeze)` by resolving all loaded
/// `TagFile` assets against the Bevy `Assets<T>` collection.
#[derive(Resource)]
pub struct Tags<T: Asset> {
    inner: HashMap<ResourceLocation<Arc<str>>, HashSet<AssetId<T>>>,
    /// Pending handles loaded by `request()`, drained at WorldgenFreeze.
    handles: HashMap<ResourceLocation<Arc<str>>, Handle<TagFile>>,
}

impl<T: Asset> Default for Tags<T> {
    fn default() -> Self {
        Tags {
            inner: HashMap::new(),
            handles: HashMap::new(),
        }
    }
}

impl<T: Asset> Tags<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a resolved tag set (called during the freeze phase).
    pub fn insert(&mut self, loc: ResourceLocation<Arc<str>>, ids: HashSet<AssetId<T>>) {
        self.inner.insert(loc, ids);
    }

    /// Check whether `id` is a member of the given tag.
    pub fn contains(&self, tag_str: &str, id: AssetId<T>) -> bool {
        self.inner
            .get(tag_str)
            .map_or(false, |set| set.contains(&id))
    }

    /// Return the full set of asset IDs for a tag, or `None` if not loaded.
    pub fn get(&self, tag_str: &str) -> Option<&HashSet<AssetId<T>>> {
        self.inner.get(tag_str)
    }

    /// Iterate over all (tag RL, id set) pairs.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &HashSet<AssetId<T>>)> {
        self.inner.iter()
    }
}

impl<T: Asset + TagRegistryType> Tags<T> {
    /// Request a tag to be loaded. Call once per tag during LoadingDataPack.
    ///
    /// No-op if the tag was already requested.
    pub fn request(&mut self, key: &TagKey<T>, asset_server: &AssetServer) {
        let loc_str = key.resource_location().as_static_str();
        if self.handles.contains_key(loc_str) {
            return;
        }
        let segment = T::REGISTRY_PATH.to_string();
        let handle = asset_server
            .load_with_settings::<TagFile, TagFileSettings>(key.asset_path(), move |s| {
                s.registry_segment = segment.clone()
            });
        self.handles.insert(key.resource_location_arc(), handle);
    }

    /// Drain all pending tag handles. Call at WorldgenFreeze to get handles for resolution.
    pub fn drain_handles(&mut self) -> Vec<(ResourceLocation<Arc<str>>, Handle<TagFile>)> {
        self.handles.drain().collect()
    }

    /// Returns `true` once every pending handle (and all its recursive dependencies)
    /// is fully loaded by Bevy's asset system.
    pub fn all_handles_loaded(&self, asset_server: &AssetServer) -> bool {
        self.handles
            .values()
            .all(|h| asset_server.is_loaded_with_dependencies(h.id()))
    }

    /// Number of tags still pending resolution (not yet drained).
    pub fn pending_handles_count(&self) -> usize {
        self.handles.len()
    }
}
