use crate::registry::StaticId;
use crate::resource_location::ResourceLocation;
use crate::tag::file::{TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TagRegistryType};
use bevy_asset::{AssetServer, Handle};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Resolved tags for static registry types (blocks, items).
///
/// Acts as both the pending-handles store (Startup) and the resolved-ids store
/// (after `OnEnter(AppState::WorldgenFreeze)`).
///
/// Internal storage uses `ResourceLocation<Arc<str>>` keys. Lookups accept
/// `&str` via `Borrow<str>` for zero-allocation access from any
/// `ResourceLocation` variant.
#[derive(Resource)]
pub struct StaticTags<T: TagRegistryType + 'static> {
    inner: HashMap<ResourceLocation<Arc<str>>, HashSet<StaticId<T>>>,
    /// Pending handles loaded by `request()`, drained at WorldgenFreeze.
    handles: HashMap<ResourceLocation<Arc<str>>, Handle<TagFile>>,
}

impl<T: TagRegistryType + 'static> Default for StaticTags<T> {
    fn default() -> Self {
        StaticTags {
            inner: HashMap::new(),
            handles: HashMap::new(),
        }
    }
}

impl<T: TagRegistryType + 'static> StaticTags<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request a tag to be loaded. Call once per tag during plugin Startup.
    ///
    /// No-op if the tag was already requested. Loading uses `TagFileSettings`
    /// so the loader can resolve nested `#tag` references correctly.
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

    /// Insert a resolved tag set (called during the WorldgenFreeze phase).
    pub fn insert(&mut self, loc: ResourceLocation<Arc<str>>, ids: HashSet<StaticId<T>>) {
        self.inner.insert(loc, ids);
    }

    /// Check whether `id` is a member of the given tag (typed `TagKey`).
    /// Zero-alloc: uses `Borrow<str>` for lookup.
    pub fn contains(&self, tag: &TagKey<T>, id: StaticId<T>) -> bool {
        self.inner
            .get(tag.resource_location().as_str())
            .map_or(false, |set| set.contains(&id))
    }

    /// Check whether `id` is a member of the tag identified by its string key.
    pub fn contains_rl(&self, tag_str: &str, id: StaticId<T>) -> bool {
        self.inner
            .get(tag_str)
            .map_or(false, |set| set.contains(&id))
    }

    /// Return the full set of IDs for a tag (typed `TagKey`), or `None` if not loaded.
    /// Zero-alloc: uses `Borrow<str>` for lookup.
    pub fn get(&self, tag: &TagKey<T>) -> Option<&HashSet<StaticId<T>>> {
        self.inner.get(tag.resource_location().as_str())
    }

    /// Return the full set of IDs for the tag identified by its string key.
    pub fn get_rl(&self, tag_str: &str) -> Option<&HashSet<StaticId<T>>> {
        self.inner.get(tag_str)
    }

    /// Number of tags still pending resolution (not yet drained).
    pub fn pending_handles_count(&self) -> usize {
        self.handles.len()
    }

    /// Returns `true` once every pending handle (and all its recursive dependencies)
    /// is fully loaded by Bevy's asset system.
    pub fn all_handles_loaded(&self, asset_server: &AssetServer) -> bool {
        self.handles
            .values()
            .all(|h| asset_server.is_loaded_with_dependencies(h.id()))
    }

    /// Returns `true` if no tags have been resolved yet.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over all resolved (tag RL, id set) pairs.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &HashSet<StaticId<T>>)> {
        self.inner.iter()
    }
}
