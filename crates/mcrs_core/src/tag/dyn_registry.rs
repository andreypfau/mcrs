use crate::resource_location::ResourceLocation;
use crate::tag::file::{TagEntry, TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TaggedRegistry};
use bevy_asset::{AssetServer, Assets, Handle};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

// ─── RawBitSet ──────────────────────────────────────────────────────────────

/// A compact bitset indexed by plain `u32`, without `StaticId<T>` coupling.
///
/// Same word-indexed bit manipulation as `IdBitSet` but operates on raw `u32`
/// values instead of typed `StaticId<T>`.
pub struct RawBitSet {
    words: Vec<u64>,
    len: u32,
}

impl RawBitSet {
    pub fn with_capacity(cap: u32) -> Self {
        let num_words = ((cap as usize) + 63) / 64;
        Self {
            words: vec![0u64; num_words],
            len: 0,
        }
    }

    pub fn insert(&mut self, id: u32) {
        let idx = id as usize;
        let (word, bit) = (idx / 64, idx % 64);
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let mask = 1u64 << bit;
        if self.words[word] & mask == 0 {
            self.words[word] |= mask;
            self.len += 1;
        }
    }

    #[inline]
    pub fn contains(&self, id: u32) -> bool {
        let idx = id as usize;
        let (word, bit) = (idx / 64, idx % 64);
        word < self.words.len() && (self.words[word] & (1u64 << bit)) != 0
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn from_hash_set(set: &HashSet<u32>, capacity: u32) -> Self {
        let mut bs = Self::with_capacity(capacity);
        for &id in set {
            bs.insert(id);
        }
        bs
    }

    pub fn iter(&self) -> RawBitSetIter<'_> {
        RawBitSetIter {
            words: &self.words,
            word_idx: 0,
            current: if self.words.is_empty() {
                0
            } else {
                self.words[0]
            },
        }
    }
}

pub struct RawBitSetIter<'a> {
    words: &'a [u64],
    word_idx: usize,
    current: u64,
}

impl Iterator for RawBitSetIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let tz = self.current.trailing_zeros();
                self.current &= self.current - 1;
                return Some((self.word_idx * 64 + tz as usize) as u32);
            }
            self.word_idx += 1;
            if self.word_idx >= self.words.len() {
                return None;
            }
            self.current = self.words[self.word_idx];
        }
    }
}

// ─── DynRegistryIndex ───────────────────────────────────────────────────────

/// A dense `ResourceLocation`-to-`u32` index for dynamic registry types.
///
/// Sorts entries alphabetically by full `namespace:path` string and assigns
/// dense 0..N indices. This deterministic ordering is reusable by
/// `RegistrySnapshot` for stable network IDs.
pub struct DynRegistryIndex<T: TaggedRegistry> {
    map: HashMap<ResourceLocation<Arc<str>>, u32>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: TaggedRegistry> DynRegistryIndex<T> {
    pub fn build(entries: impl Iterator<Item = ResourceLocation<Arc<str>>>) -> Self {
        let mut sorted: Vec<ResourceLocation<Arc<str>>> = entries.collect();
        sorted.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let map = sorted
            .into_iter()
            .enumerate()
            .map(|(i, rl)| (rl, i as u32))
            .collect();
        Self {
            map,
            _marker: PhantomData,
        }
    }

    pub fn get(&self, rl: &str) -> Option<u32> {
        self.map.get(rl).copied()
    }

    pub fn len(&self) -> u32 {
        self.map.len() as u32
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ─── DynTagRegistry ─────────────────────────────────────────────────────────

struct DynLoadingState<T: TaggedRegistry + 'static> {
    inner: HashMap<ResourceLocation<Arc<str>>, HashSet<u32>>,
    handles: HashMap<ResourceLocation<Arc<str>>, Handle<TagFile>>,
    _marker: PhantomData<fn() -> T>,
}

/// Tag registry for dynamic registry types backed by `Assets<T>`.
///
/// Mirrors `TagRegistry<T>` lifecycle but resolves entries against a
/// `DynRegistryIndex<T>` (ResourceLocation-to-u32 dense index) instead of
/// `StaticRegistry<T>`.
#[derive(Resource)]
pub struct DynTagRegistry<T: TaggedRegistry + 'static> {
    loading: Option<DynLoadingState<T>>,
    index: HashMap<ResourceLocation<Arc<str>>, usize>,
    bitsets: Vec<RawBitSet>,
}

impl<T: TaggedRegistry + 'static> Default for DynTagRegistry<T> {
    fn default() -> Self {
        Self {
            loading: Some(DynLoadingState {
                inner: HashMap::new(),
                handles: HashMap::new(),
                _marker: PhantomData,
            }),
            index: HashMap::new(),
            bitsets: Vec::new(),
        }
    }
}

impl<T: TaggedRegistry + 'static> DynTagRegistry<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request a tag to be loaded. Call once per tag during plugin Startup.
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

    /// Drain all pending tag handles for resolution.
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
    pub fn insert(&mut self, loc: ResourceLocation<Arc<str>>, ids: HashSet<u32>) {
        let m = self
            .loading
            .as_mut()
            .expect("insert() called after freeze()");
        m.inner.insert(loc, ids);
    }

    /// Convert all `HashSet<u32>` tag data into dense `RawBitSet` storage.
    ///
    /// # Panics
    /// Panics if called twice.
    pub fn freeze(&mut self, registry_index: &DynRegistryIndex<T>) {
        let m = self.loading.take().expect("freeze() called twice");
        let capacity = registry_index.len();
        let mut index = HashMap::with_capacity(m.inner.len());
        let mut bitsets = Vec::with_capacity(m.inner.len());
        for (loc, set) in m.inner {
            let slot = bitsets.len();
            bitsets.push(RawBitSet::from_hash_set(&set, capacity));
            index.insert(loc, slot);
        }
        self.index = index;
        self.bitsets = bitsets;
    }

    /// Check whether `entry_id` is a member of the given tag.
    ///
    /// After `freeze()`: HashMap string lookup + bit test.
    /// Before `freeze()`: HashMap string lookup + HashSet probe (fallback).
    pub fn contains<S: AsRef<str>>(&self, tag: &TagKey<T, S>, entry_id: u32) -> bool {
        if let Some(m) = &self.loading {
            m.inner
                .get(tag.as_str())
                .map_or(false, |set| set.contains(&entry_id))
        } else {
            self.index
                .get(tag.as_str())
                .map_or(false, |&slot| self.bitsets[slot].contains(entry_id))
        }
    }

    /// Return the bitset for a tag, or `None` if not loaded / not frozen.
    pub fn get<S: AsRef<str>>(&self, tag: &TagKey<T, S>) -> Option<&RawBitSet> {
        let &slot = self.index.get(tag.as_str())?;
        Some(&self.bitsets[slot])
    }

    /// Returns `true` once every pending handle is fully loaded.
    pub fn all_handles_loaded(&self, asset_server: &AssetServer) -> bool {
        match &self.loading {
            Some(m) => m
                .handles
                .values()
                .all(|h| asset_server.is_loaded_with_dependencies(h.id())),
            None => true,
        }
    }

    pub fn is_empty(&self) -> bool {
        if let Some(m) = &self.loading {
            m.inner.is_empty()
        } else {
            self.bitsets.is_empty()
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.loading.is_none()
    }

    /// Iterate over all resolved (tag RL, bitset) pairs. Only available after `freeze()`.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &RawBitSet)> {
        self.index
            .iter()
            .map(|(loc, &slot)| (loc, &self.bitsets[slot]))
    }

    /// Resolve a tag file and insert the result in one step.
    ///
    /// # Panics
    /// Panics if called after `freeze()`.
    pub fn resolve_and_insert(
        &mut self,
        loc: ResourceLocation<Arc<str>>,
        tag_file: &TagFile,
        all_files: &Assets<TagFile>,
        index: &DynRegistryIndex<T>,
    ) {
        let ids = resolve_dyn_tag_file::<T>(tag_file, all_files, index);
        self.insert(loc, ids);
    }
}

/// Recursively expand a `TagFile` into a set of `u32` IDs using a
/// `DynRegistryIndex` for element resolution.
pub fn resolve_dyn_tag_file<T: TaggedRegistry>(
    tag_file: &TagFile,
    all_files: &Assets<TagFile>,
    index: &DynRegistryIndex<T>,
) -> HashSet<u32> {
    let mut out = HashSet::new();
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) => {
                if let Some(id) = index.get(loc.as_str()) {
                    out.insert(id);
                } else {
                    tracing::warn!("tag references unknown dynamic registry entry: {}", loc.as_str());
                }
            }
            TagEntry::OptionalElement(loc) => {
                if let Some(id) = index.get(loc.as_str()) {
                    out.insert(id);
                }
            }
            TagEntry::Tag(h) | TagEntry::OptionalTag(h) => {
                if let Some(nested) = all_files.get(h) {
                    out.extend(resolve_dyn_tag_file::<T>(nested, all_files, index));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_location::ResourceLocation;
    use crate::tag::key::TaggedRegistry;

    struct TestBiome;
    impl TaggedRegistry for TestBiome {
        const REGISTRY_PATH: &'static str = "worldgen/biome";
    }

    fn tag(s: &'static str) -> TagKey<TestBiome> {
        TagKey::new(ResourceLocation::new_static(s))
    }

    fn rl_arc(s: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(s).unwrap()
    }

    // ── DynRegistryIndex ──

    #[test]
    fn index_build_produces_dense_mapping() {
        let entries = vec![
            rl_arc("minecraft:plains"),
            rl_arc("minecraft:desert"),
            rl_arc("minecraft:forest"),
        ];
        let index = DynRegistryIndex::<TestBiome>::build(entries.into_iter());
        assert_eq!(index.len(), 3);
        assert_eq!(index.get("minecraft:desert"), Some(0));
        assert_eq!(index.get("minecraft:forest"), Some(1));
        assert_eq!(index.get("minecraft:plains"), Some(2));
    }

    #[test]
    fn index_get_missing_returns_none() {
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        assert_eq!(index.get("minecraft:nonexistent"), None);
    }

    #[test]
    fn index_empty() {
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    // ── RawBitSet ──

    #[test]
    fn raw_bitset_empty() {
        let bs = RawBitSet::with_capacity(128);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert!(!bs.contains(0));
    }

    #[test]
    fn raw_bitset_insert_and_contains() {
        let mut bs = RawBitSet::with_capacity(256);
        bs.insert(0);
        bs.insert(63);
        bs.insert(64);
        bs.insert(200);
        assert_eq!(bs.len(), 4);
        assert!(bs.contains(0));
        assert!(bs.contains(63));
        assert!(bs.contains(64));
        assert!(bs.contains(200));
        assert!(!bs.contains(1));
        assert!(!bs.contains(65));
    }

    #[test]
    fn raw_bitset_duplicate_insert() {
        let mut bs = RawBitSet::with_capacity(64);
        bs.insert(10);
        assert_eq!(bs.len(), 1);
        bs.insert(10);
        assert_eq!(bs.len(), 1);
    }

    #[test]
    fn raw_bitset_from_hash_set() {
        let mut set = HashSet::new();
        set.insert(5u32);
        set.insert(10);
        set.insert(100);
        let bs = RawBitSet::from_hash_set(&set, 128);
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(5));
        assert!(bs.contains(10));
        assert!(bs.contains(100));
        assert!(!bs.contains(0));
    }

    #[test]
    fn raw_bitset_iter() {
        let mut bs = RawBitSet::with_capacity(256);
        bs.insert(3);
        bs.insert(7);
        bs.insert(128);
        let got: Vec<u32> = bs.iter().collect();
        assert_eq!(got, vec![3, 7, 128]);
    }

    #[test]
    fn raw_bitset_contains_out_of_range() {
        let bs = RawBitSet::with_capacity(64);
        assert!(!bs.contains(9999));
    }

    #[test]
    fn raw_bitset_zero_capacity() {
        let bs = RawBitSet::with_capacity(0);
        assert!(bs.is_empty());
        assert!(!bs.contains(0));
        assert_eq!(bs.iter().count(), 0);
    }

    // ── DynTagRegistry lifecycle ──

    #[test]
    fn new_is_empty_and_not_frozen() {
        let reg = DynTagRegistry::<TestBiome>::new();
        assert!(reg.is_empty());
        assert!(!reg.is_frozen());
    }

    #[test]
    fn insert_and_contains_before_freeze() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let mut set = HashSet::new();
        set.insert(0u32);
        set.insert(5);
        reg.insert(rl_arc("minecraft:is_forest"), set);

        assert!(!reg.is_empty());
        let t = tag("minecraft:is_forest");
        assert!(reg.contains(&t, 0));
        assert!(reg.contains(&t, 5));
        assert!(!reg.contains(&t, 1));
    }

    #[test]
    fn contains_unknown_tag_returns_false() {
        let reg = DynTagRegistry::<TestBiome>::new();
        let t = tag("minecraft:nonexistent");
        assert!(!reg.contains(&t, 0));
    }

    #[test]
    fn freeze_converts_to_bitset() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let mut set = HashSet::new();
        set.insert(0u32);
        set.insert(2);
        reg.insert(rl_arc("minecraft:is_forest"), set);

        let entries = vec![
            rl_arc("minecraft:desert"),
            rl_arc("minecraft:forest"),
            rl_arc("minecraft:plains"),
        ];
        let index = DynRegistryIndex::<TestBiome>::build(entries.into_iter());
        reg.freeze(&index);

        assert!(reg.is_frozen());
        let t = tag("minecraft:is_forest");
        assert!(reg.contains(&t, 0));
        assert!(reg.contains(&t, 2));
        assert!(!reg.contains(&t, 1));
    }

    #[test]
    fn get_returns_bitset_after_freeze() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let mut set = HashSet::new();
        set.insert(1u32);
        set.insert(2);
        reg.insert(rl_arc("minecraft:is_forest"), set);

        let t = tag("minecraft:is_forest");
        assert!(reg.get(&t).is_none());

        let index = DynRegistryIndex::<TestBiome>::build(
            vec![rl_arc("minecraft:a"), rl_arc("minecraft:b"), rl_arc("minecraft:c")].into_iter(),
        );
        reg.freeze(&index);

        let bs = reg.get(&t).expect("tag should exist after freeze");
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(1));
        assert!(bs.contains(2));
        assert!(!bs.contains(0));
    }

    #[test]
    fn iter_yields_all_tags_after_freeze() {
        let mut reg = DynTagRegistry::<TestBiome>::new();

        let mut set1 = HashSet::new();
        set1.insert(0u32);
        reg.insert(rl_arc("minecraft:is_forest"), set1);

        let mut set2 = HashSet::new();
        set2.insert(1u32);
        reg.insert(rl_arc("minecraft:is_ocean"), set2);

        let entries = vec![rl_arc("minecraft:forest"), rl_arc("minecraft:ocean")];
        let index = DynRegistryIndex::<TestBiome>::build(entries.into_iter());
        reg.freeze(&index);

        let mut names: Vec<String> = reg.iter().map(|(rl, _)| rl.as_str().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["minecraft:is_forest", "minecraft:is_ocean"]);
    }

    #[test]
    fn iter_empty_after_freeze() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        reg.freeze(&index);
        assert_eq!(reg.iter().count(), 0);
    }

    #[test]
    fn is_empty_after_freeze_with_tags() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let mut set = HashSet::new();
        set.insert(0u32);
        reg.insert(rl_arc("minecraft:test"), set);
        let index = DynRegistryIndex::<TestBiome>::build(
            vec![rl_arc("minecraft:a")].into_iter(),
        );
        reg.freeze(&index);
        assert!(!reg.is_empty());
    }

    // ── Panic guards ──

    #[test]
    #[should_panic(expected = "freeze() called twice")]
    fn double_freeze_panics() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        reg.freeze(&index);
        reg.freeze(&index);
    }

    #[test]
    #[should_panic(expected = "insert() called after freeze()")]
    fn insert_after_freeze_panics() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        reg.freeze(&index);
        reg.insert(rl_arc("minecraft:test"), HashSet::new());
    }

    #[test]
    #[should_panic(expected = "drain_handles() called after freeze()")]
    fn drain_handles_after_freeze_panics() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        reg.freeze(&index);
        let _ = reg.drain_handles();
    }

    // ── resolve_dyn_tag_file ──

    #[test]
    fn resolve_dyn_tag_file_element() {
        let entries = vec![
            rl_arc("minecraft:desert"),
            rl_arc("minecraft:forest"),
            rl_arc("minecraft:plains"),
        ];
        let index = DynRegistryIndex::<TestBiome>::build(entries.into_iter());

        let tag_file = TagFile {
            replace: false,
            values: vec![
                TagEntry::Element(rl_arc("minecraft:forest")),
                TagEntry::Element(rl_arc("minecraft:plains")),
            ],
        };

        let all_files = Assets::<TagFile>::default();
        let result = resolve_dyn_tag_file::<TestBiome>(&tag_file, &all_files, &index);
        assert!(result.contains(&1)); // forest
        assert!(result.contains(&2)); // plains
        assert!(!result.contains(&0)); // desert not included
    }

    #[test]
    fn resolve_dyn_tag_file_optional_element_missing() {
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());

        let tag_file = TagFile {
            replace: false,
            values: vec![TagEntry::OptionalElement(rl_arc("minecraft:nonexistent"))],
        };

        let all_files = Assets::<TagFile>::default();
        let result = resolve_dyn_tag_file::<TestBiome>(&tag_file, &all_files, &index);
        assert!(result.is_empty());
    }

    // ── Consistency: pre-freeze vs post-freeze ──

    #[test]
    fn contains_matches_before_and_after_freeze() {
        let mut reg = DynTagRegistry::<TestBiome>::new();
        let ids: Vec<u32> = vec![0, 1, 15, 63, 64, 100, 127, 255, 500, 999];
        let mut set = HashSet::new();
        for &raw in &ids {
            set.insert(raw);
        }
        reg.insert(rl_arc("minecraft:big_tag"), set);

        let t = tag("minecraft:big_tag");

        for raw in 0..1024 {
            let expected = ids.contains(&raw);
            assert_eq!(
                reg.contains(&t, raw),
                expected,
                "pre-freeze mismatch at id {raw}"
            );
        }

        let entries: Vec<_> = (0..1024u32)
            .map(|i| rl_arc(&format!("minecraft:entry_{i:04}")))
            .collect();
        let index = DynRegistryIndex::<TestBiome>::build(entries.into_iter());
        reg.freeze(&index);

        for raw in 0..1024 {
            let expected = ids.contains(&raw);
            assert_eq!(
                reg.contains(&t, raw),
                expected,
                "post-freeze mismatch at id {raw}"
            );
        }
    }
}
