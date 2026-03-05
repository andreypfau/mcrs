use crate::registry::{StaticId, StaticRegistry};
use crate::resource_location::ResourceLocation;
use crate::tag::bitset::IdBitSet;
use crate::tag::file::{TagEntry, TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TaggedRegistry};
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The tag registry for static registry types (blocks, items).
///
/// Two-phase lifecycle:
/// - **Loading** (`LoadingState`): mutable `HashMap<RL, HashSet<StaticId<T>>>`
///   for requesting tag files and inserting resolved entries.
/// - **Frozen**: immutable `HashMap<RL, usize>` index into a dense
///   `Vec<IdBitSet<T>>`. Membership tests become a single bit check.
///
/// Internal storage uses `ResourceLocation<Arc<str>>` keys. Lookups accept
/// `&str` via `Borrow<str>` for zero-allocation access from any
/// `ResourceLocation` variant.
#[derive(Resource)]
pub struct TagRegistry<T: TaggedRegistry + 'static> {
    /// Loading state (request + insert). `None` after `freeze()`.
    loading: Option<LoadingState<T>>,
    /// Post-freeze: tag RL → index into `bitsets`.
    index: HashMap<ResourceLocation<Arc<str>>, usize>,
    /// Post-freeze: dense bitset storage, indexed by tag slot.
    bitsets: Vec<IdBitSet<T>>,
}

struct LoadingState<T: TaggedRegistry + 'static> {
    inner: HashMap<ResourceLocation<Arc<str>>, HashSet<StaticId<T>>>,
    handles: HashMap<ResourceLocation<Arc<str>>, Handle<TagFile>>,
}

impl<T: TaggedRegistry + 'static> Default for TagRegistry<T> {
    fn default() -> Self {
        TagRegistry {
            loading: Some(LoadingState {
                inner: HashMap::new(),
                handles: HashMap::new(),
            }),
            index: HashMap::new(),
            bitsets: Vec::new(),
        }
    }
}

impl<T: TaggedRegistry + 'static> TagRegistry<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request a tag to be loaded. Call once per tag during plugin Startup.
    ///
    /// No-op if the tag was already requested. Loading uses `TagFileSettings`
    /// so the loader can resolve nested `#tag` references correctly.
    ///
    /// Generic over `S` so both static (`TagKey<T>`) and runtime
    /// (`TagKey<T, Arc<str>>`) tag keys can be used.
    ///
    /// # Panics
    /// Panics if called after `freeze()`.
    pub fn request<S: AsRef<str>>(&mut self, key: &TagKey<T, S>, asset_server: &AssetServer) {
        let m = self
            .loading
            .as_mut()
            .expect("request() called after freeze()");
        if m.handles.contains_key(key.as_str()) {
            return;
        }
        let segment = T::REGISTRY_PATH.to_string();
        let handle = asset_server
            .load_with_settings::<TagFile, TagFileSettings>(key.asset_path(), move |s| {
                s.registry_segment = segment.clone()
            });
        m.handles.insert(key.to_arc().location().clone(), handle);
    }

    /// Drain all pending tag handles. Call at WorldgenFreeze to get handles for resolution.
    ///
    /// # Panics
    /// Panics if called after `freeze()`.
    pub fn drain_handles(&mut self) -> Vec<(ResourceLocation<Arc<str>>, Handle<TagFile>)> {
        let m = self
            .loading
            .as_mut()
            .expect("drain_handles() called after freeze()");
        m.handles.drain().collect()
    }

    /// Insert a resolved tag set (called during the WorldgenFreeze phase).
    ///
    /// # Panics
    /// Panics if called after `freeze()`.
    pub fn insert(&mut self, loc: ResourceLocation<Arc<str>>, ids: HashSet<StaticId<T>>) {
        let m = self
            .loading
            .as_mut()
            .expect("insert() called after freeze()");
        m.inner.insert(loc, ids);
    }

    /// Convert all `HashSet` tag data into dense `IdBitSet` storage.
    ///
    /// After this call, `contains()` uses a bit test instead of a hash lookup.
    /// `request()`, `drain_handles()`, and `insert()` will panic.
    pub fn freeze(&mut self, registry_len: u32) {
        let m = self.loading.take().expect("freeze() called twice");
        let mut index = HashMap::with_capacity(m.inner.len());
        let mut bitsets = Vec::with_capacity(m.inner.len());
        for (loc, set) in m.inner {
            let slot = bitsets.len();
            bitsets.push(IdBitSet::from_hash_set(&set, registry_len));
            index.insert(loc, slot);
        }
        self.index = index;
        self.bitsets = bitsets;
    }

    /// Check whether `id` is a member of the given tag (typed `TagKey`).
    /// Zero-alloc: uses `Borrow<str>` for lookup.
    ///
    /// After `freeze()`: HashMap string lookup + bit test.
    /// Before `freeze()`: HashMap string lookup + HashSet probe (fallback).
    pub fn contains<S: AsRef<str>>(&self, tag: &TagKey<T, S>, id: StaticId<T>) -> bool {
        if let Some(m) = &self.loading {
            // Pre-freeze fallback.
            m.inner
                .get(tag.as_str())
                .map_or(false, |set| set.contains(&id))
        } else {
            // Post-freeze fast path.
            self.index
                .get(tag.as_str())
                .map_or(false, |&slot| self.bitsets[slot].contains(id))
        }
    }

    /// Return the bitset for a tag, or `None` if not loaded / not frozen.
    pub fn get<S: AsRef<str>>(&self, tag: &TagKey<T, S>) -> Option<&IdBitSet<T>> {
        let &slot = self.index.get(tag.as_str())?;
        Some(&self.bitsets[slot])
    }

    /// Number of tags still pending resolution (not yet drained).
    pub fn pending_handles_count(&self) -> usize {
        self.loading.as_ref().map_or(0, |m| m.handles.len())
    }

    /// Returns `true` once every pending handle (and all its recursive dependencies)
    /// is fully loaded by Bevy's asset system.
    pub fn all_handles_loaded(&self, asset_server: &AssetServer) -> bool {
        match &self.loading {
            Some(m) => m
                .handles
                .values()
                .all(|h| asset_server.is_loaded_with_dependencies(h.id())),
            None => true,
        }
    }

    /// Returns `true` if no tags have been resolved yet.
    pub fn is_empty(&self) -> bool {
        if let Some(m) = &self.loading {
            m.inner.is_empty()
        } else {
            self.bitsets.is_empty()
        }
    }

    /// Returns `true` if `freeze()` has been called.
    pub fn is_frozen(&self) -> bool {
        self.loading.is_none()
    }

    /// Iterate over all resolved (tag RL, bitset) pairs.
    /// Only available after `freeze()`.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &IdBitSet<T>)> {
        self.index
            .iter()
            .map(|(loc, &slot)| (loc, &self.bitsets[slot]))
    }

    /// Recursively expand a `TagFile` into a set of `StaticId<T>`.
    ///
    /// Resolves `#tag` references by following nested tag file handles,
    /// and plain element references by looking up the static registry.
    pub fn resolve_tag_file(
        tag_file: &TagFile,
        all_files: &Assets<TagFile>,
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
                    if let Some(nested) = all_files.get(h) {
                        out.extend(Self::resolve_tag_file(nested, all_files, registry));
                    }
                }
            }
        }
        out
    }

    /// Resolve a tag file and insert the result into this registry in one step.
    ///
    /// Convenience wrapper around [`Self::resolve_tag_file`] + [`Self::insert`].
    ///
    /// # Panics
    /// Panics if called after `freeze()`.
    pub fn resolve_and_insert(
        &mut self,
        loc: ResourceLocation<Arc<str>>,
        tag_file: &TagFile,
        all_files: &Assets<TagFile>,
        registry: &StaticRegistry<T>,
    ) {
        let ids = Self::resolve_tag_file(tag_file, all_files, registry);
        self.insert(loc, ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::StaticId;
    use crate::tag::key::TaggedRegistry;

    /// Dummy registry element type for testing.
    struct TestBlock;
    impl TaggedRegistry for TestBlock {
        const REGISTRY_PATH: &'static str = "block";
    }

    fn id(raw: u32) -> StaticId<TestBlock> {
        StaticId::new(raw)
    }

    fn tag(s: &'static str) -> TagKey<TestBlock> {
        TagKey::new(ResourceLocation::new_static(s))
    }

    fn rl_arc(s: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(s).unwrap()
    }

    // ── Basic lifecycle: new → insert → contains → freeze → contains ──

    #[test]
    fn new_is_empty_and_not_frozen() {
        let st = TagRegistry::<TestBlock>::new();
        assert!(st.is_empty());
        assert!(!st.is_frozen());
    }

    #[test]
    fn insert_and_contains_before_freeze() {
        let mut st = TagRegistry::<TestBlock>::new();
        let mut set = HashSet::new();
        set.insert(id(0));
        set.insert(id(5));
        set.insert(id(10));
        st.insert(rl_arc("minecraft:mineable/pickaxe"), set);

        assert!(!st.is_empty());
        assert!(!st.is_frozen());

        let tag = tag("minecraft:mineable/pickaxe");
        assert!(st.contains(&tag, id(0)));
        assert!(st.contains(&tag, id(5)));
        assert!(st.contains(&tag, id(10)));
        assert!(!st.contains(&tag, id(1)));
        assert!(!st.contains(&tag, id(99)));
    }

    #[test]
    fn contains_unknown_tag_returns_false() {
        let st = TagRegistry::<TestBlock>::new();
        let tag = tag("minecraft:nonexistent");
        assert!(!st.contains(&tag, id(0)));
    }

    #[test]
    fn freeze_converts_to_bitset() {
        let mut st = TagRegistry::<TestBlock>::new();
        let mut set = HashSet::new();
        set.insert(id(2));
        set.insert(id(7));
        set.insert(id(63));
        set.insert(id(64));
        st.insert(rl_arc("minecraft:logs"), set);

        st.freeze(128);

        assert!(st.is_frozen());
        assert!(!st.is_empty());

        let tag = tag("minecraft:logs");
        assert!(st.contains(&tag, id(2)));
        assert!(st.contains(&tag, id(7)));
        assert!(st.contains(&tag, id(63)));
        assert!(st.contains(&tag, id(64)));
        assert!(!st.contains(&tag, id(0)));
        assert!(!st.contains(&tag, id(65)));
        assert!(!st.contains(&tag, id(127)));
    }

    #[test]
    fn get_returns_bitset_after_freeze() {
        let mut st = TagRegistry::<TestBlock>::new();
        let mut set = HashSet::new();
        set.insert(id(3));
        set.insert(id(42));
        st.insert(rl_arc("minecraft:sand"), set);

        // get() returns None before freeze (only index is populated after freeze).
        let tag = tag("minecraft:sand");
        assert!(st.get(&tag).is_none());

        st.freeze(64);

        let bs = st.get(&tag).expect("tag should exist after freeze");
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(id(3)));
        assert!(bs.contains(id(42)));
        assert!(!bs.contains(id(0)));
    }

    #[test]
    fn get_unknown_tag_returns_none() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        let tag = tag("minecraft:nope");
        assert!(st.get(&tag).is_none());
    }

    // ── Multiple tags ──

    #[test]
    fn multiple_tags_independent() {
        let mut st = TagRegistry::<TestBlock>::new();

        let mut set_a = HashSet::new();
        set_a.insert(id(1));
        set_a.insert(id(2));
        st.insert(rl_arc("minecraft:logs"), set_a);

        let mut set_b = HashSet::new();
        set_b.insert(id(2));
        set_b.insert(id(3));
        st.insert(rl_arc("minecraft:leaves"), set_b);

        st.freeze(64);

        let logs = tag("minecraft:logs");
        let leaves = tag("minecraft:leaves");

        assert!(st.contains(&logs, id(1)));
        assert!(st.contains(&logs, id(2)));
        assert!(!st.contains(&logs, id(3)));

        assert!(!st.contains(&leaves, id(1)));
        assert!(st.contains(&leaves, id(2)));
        assert!(st.contains(&leaves, id(3)));
    }

    // ── iter() ──

    #[test]
    fn iter_yields_all_tags() {
        let mut st = TagRegistry::<TestBlock>::new();

        let mut set1 = HashSet::new();
        set1.insert(id(10));
        st.insert(rl_arc("minecraft:wool"), set1);

        let mut set2 = HashSet::new();
        set2.insert(id(20));
        st.insert(rl_arc("minecraft:snow"), set2);

        st.freeze(64);

        let mut tag_names: Vec<String> = st.iter().map(|(rl, _)| rl.as_str().to_string()).collect();
        tag_names.sort();
        assert_eq!(tag_names, vec!["minecraft:snow", "minecraft:wool"]);

        // Verify each bitset has the right content.
        for (rl, bs) in st.iter() {
            match rl.as_str() {
                "minecraft:wool" => {
                    assert_eq!(bs.len(), 1);
                    assert!(bs.contains(id(10)));
                }
                "minecraft:snow" => {
                    assert_eq!(bs.len(), 1);
                    assert!(bs.contains(id(20)));
                }
                other => panic!("unexpected tag: {other}"),
            }
        }
    }

    #[test]
    fn iter_empty_after_freeze() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        assert_eq!(st.iter().count(), 0);
    }

    // ── is_empty ──

    #[test]
    fn is_empty_after_freeze_with_no_tags() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        assert!(st.is_empty());
    }

    #[test]
    fn is_empty_false_after_freeze_with_tags() {
        let mut st = TagRegistry::<TestBlock>::new();
        let mut set = HashSet::new();
        set.insert(id(0));
        st.insert(rl_arc("minecraft:test"), set);
        st.freeze(64);
        assert!(!st.is_empty());
    }

    // ── Panic guards ──

    #[test]
    #[should_panic(expected = "freeze() called twice")]
    fn double_freeze_panics() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        st.freeze(64);
    }

    #[test]
    #[should_panic(expected = "insert() called after freeze()")]
    fn insert_after_freeze_panics() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        st.insert(rl_arc("minecraft:test"), HashSet::new());
    }

    #[test]
    #[should_panic(expected = "drain_handles() called after freeze()")]
    fn drain_handles_after_freeze_panics() {
        let mut st = TagRegistry::<TestBlock>::new();
        st.freeze(64);
        let _ = st.drain_handles();
    }

    // ── Consistency: pre-freeze HashSet vs post-freeze bitset ──

    #[test]
    fn contains_matches_before_and_after_freeze() {
        let mut st = TagRegistry::<TestBlock>::new();
        let ids: Vec<u32> = vec![0, 1, 15, 63, 64, 100, 127, 255, 500, 999];
        let mut set = HashSet::new();
        for &raw in &ids {
            set.insert(id(raw));
        }
        st.insert(rl_arc("minecraft:big_tag"), set);

        let tag = tag("minecraft:big_tag");

        // Check pre-freeze.
        for raw in 0..1024 {
            let expected = ids.contains(&raw);
            assert_eq!(
                st.contains(&tag, id(raw)),
                expected,
                "pre-freeze mismatch at id {raw}"
            );
        }

        st.freeze(1024);

        // Check post-freeze — should be identical.
        for raw in 0..1024 {
            let expected = ids.contains(&raw);
            assert_eq!(
                st.contains(&tag, id(raw)),
                expected,
                "post-freeze mismatch at id {raw}"
            );
        }
    }
}
