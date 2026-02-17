use crate::resource_location::ResourceLocation;
use bevy_asset::{Asset, AssetId};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};

/// Resolved tags for dynamic asset types (biomes, dimension types, etc.).
///
/// Populated during `OnEnter(AppState::WorldgenFreeze)` by resolving all loaded
/// `TagFile` assets against the Bevy `Assets<T>` collection.
#[derive(Resource)]
pub struct Tags<T: Asset> {
    inner: HashMap<ResourceLocation, HashSet<AssetId<T>>>,
}

impl<T: Asset> Default for Tags<T> {
    fn default() -> Self {
        Tags {
            inner: HashMap::new(),
        }
    }
}

impl<T: Asset> Tags<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a resolved tag set (called during the freeze phase).
    pub fn insert(&mut self, loc: ResourceLocation, ids: HashSet<AssetId<T>>) {
        self.inner.insert(loc, ids);
    }

    /// Check whether `id` is a member of the given tag.
    pub fn contains(&self, tag_loc: &ResourceLocation, id: AssetId<T>) -> bool {
        self.inner
            .get(tag_loc)
            .map_or(false, |set| set.contains(&id))
    }

    /// Return the full set of asset IDs for a tag, or `None` if not loaded.
    pub fn get(&self, tag_loc: &ResourceLocation) -> Option<&HashSet<AssetId<T>>> {
        self.inner.get(tag_loc)
    }

    /// Iterate over all (tag RL, id set) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation, &HashSet<AssetId<T>>)> {
        self.inner.iter()
    }
}
