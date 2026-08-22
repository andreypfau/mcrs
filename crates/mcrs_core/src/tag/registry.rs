use crate::registry::{StaticId, StaticRegistry};
use crate::resource_location::ResourceLocation;
use crate::tag::bitset::{BitSet, TagId};
use crate::tag::dyn_index::DynRegistryIndex;
use crate::tag::file::{TagEntry, TagFile, TagFileSettings};
use crate::tag::key::{TagKey, TaggedRegistry};
use bevy_asset::{AssetServer, Assets, Handle, RecursiveDependencyLoadState};
use bevy_ecs::resource::Resource;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

/// The registry a tag file's element references are resolved against.
pub trait TagSource: Send + Sync + 'static {
    type Id: TagId;

    fn id_of(&self, loc: &str) -> Option<Self::Id>;

    /// Upper bound on ids, used to size the frozen bitsets.
    fn capacity(&self) -> u32;
}

impl<T: Send + Sync + 'static> TagSource for StaticRegistry<T> {
    type Id = StaticId<T>;

    fn id_of(&self, loc: &str) -> Option<StaticId<T>> {
        StaticRegistry::id_of(self, loc)
    }

    fn capacity(&self) -> u32 {
        self.len() as u32
    }
}

impl<T: TaggedRegistry + Send + Sync + 'static> TagSource for DynRegistryIndex<T> {
    type Id = u32;

    fn id_of(&self, loc: &str) -> Option<u32> {
        DynRegistryIndex::get(self, loc)
    }

    fn capacity(&self) -> u32 {
        self.len()
    }
}

/// Recursively expand a `TagFile` into the set of ids it names.
///
/// `#tag` references are followed through nested tag file handles; plain
/// element references are looked up in `source`.
pub fn resolve_tag_file<S: TagSource>(
    tag_file: &TagFile,
    all_files: &Assets<TagFile>,
    source: &S,
) -> HashSet<S::Id> {
    resolve_tag_file_ordered(tag_file, all_files, source)
        .into_iter()
        .collect()
}

/// Expand a `TagFile` in the order it lists its entries, with nested `#tag`
/// references expanded in place and repeats keeping their first position.
///
/// For tags that denote a sequence rather than a set — a dimension's
/// timelines stack in this order — which the frozen bitset cannot express.
pub fn resolve_tag_file_ordered<S: TagSource>(
    tag_file: &TagFile,
    all_files: &Assets<TagFile>,
    source: &S,
) -> Vec<S::Id> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    extend_from_tag_file(tag_file, all_files, source, &mut out, &mut seen);
    out
}

fn extend_from_tag_file<S: TagSource>(
    tag_file: &TagFile,
    all_files: &Assets<TagFile>,
    source: &S,
    out: &mut Vec<S::Id>,
    seen: &mut HashSet<S::Id>,
) {
    for entry in &tag_file.values {
        match entry {
            TagEntry::Element(loc) => match source.id_of(loc.as_str()) {
                Some(id) if seen.insert(id) => out.push(id),
                Some(_) => {}
                None => tracing::warn!("tag references unknown registry entry: {loc}"),
            },
            TagEntry::OptionalElement(loc) => {
                if let Some(id) = source.id_of(loc.as_str())
                    && seen.insert(id)
                {
                    out.push(id);
                }
            }
            TagEntry::Tag(h) | TagEntry::OptionalTag(h) => {
                if let Some(nested) = all_files.get(h) {
                    extend_from_tag_file(nested, all_files, source, out, seen);
                }
            }
        }
    }
}

/// The loading half of a tagged registry: requests tag files, collects
/// resolved membership sets, and is consumed by [`TagLoader::freeze`].
///
/// Its presence in the world *is* the loading phase — the frozen
/// [`TagRegistry`] only exists once this has been consumed, so no reader can
/// ask a membership question against half-loaded data.
#[derive(Resource)]
pub struct TagLoader<T: TaggedRegistry + 'static, I: TagId = StaticId<T>> {
    requested: &'static [TagKey<T>],
    handles: HashMap<ResourceLocation<Arc<str>>, Handle<TagFile>>,
    resolved: HashMap<ResourceLocation<Arc<str>>, HashSet<I>>,
}

pub type DynTagLoader<T> = TagLoader<T, u32>;

impl<T: TaggedRegistry + 'static, I: TagId> Default for TagLoader<T, I> {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl<T: TaggedRegistry + 'static, I: TagId> TagLoader<T, I> {
    pub fn new(requested: &'static [TagKey<T>]) -> Self {
        TagLoader {
            requested,
            handles: HashMap::new(),
            resolved: HashMap::new(),
        }
    }

    /// The tag list this loader was declared with.
    pub fn requested(&self) -> &'static [TagKey<T>] {
        self.requested
    }

    /// Request a tag file to be loaded. No-op if already requested.
    ///
    /// Loading uses `TagFileSettings` so the loader can resolve nested `#tag`
    /// references correctly.
    pub fn request<S: AsRef<str>>(&mut self, key: &TagKey<T, S>, asset_server: &AssetServer) {
        if self.handles.contains_key(key.as_str()) {
            return;
        }
        let handle = asset_server
            .load_builder()
            .with_settings(|s: &mut TagFileSettings| {
                s.registry_segment = T::REGISTRY_PATH.to_string();
            })
            .load::<TagFile>(key.asset_path());
        self.handles.insert(key.to_arc().location().clone(), handle);
    }

    /// Take all pending tag handles for resolution.
    pub fn drain_handles(&mut self) -> Vec<(ResourceLocation<Arc<str>>, Handle<TagFile>)> {
        self.handles.drain().collect()
    }

    pub fn insert(&mut self, loc: ResourceLocation<Arc<str>>, ids: HashSet<I>) {
        self.resolved.insert(loc, ids);
    }

    /// Resolve a tag file against `source` and store the result.
    pub fn resolve_and_insert<S: TagSource<Id = I>>(
        &mut self,
        loc: ResourceLocation<Arc<str>>,
        tag_file: &TagFile,
        all_files: &Assets<TagFile>,
        source: &S,
    ) {
        let ids = resolve_tag_file(tag_file, all_files, source);
        self.insert(loc, ids);
    }

    /// Returns `true` once every pending handle and everything it pulls in has
    /// either loaded or failed. Nested `#tag` references are dependencies, so
    /// waiting on the tag file alone resolves it against a half-loaded tree.
    /// Failures count as settled: a missing tag file must not stall startup,
    /// resolution warns about it instead.
    pub fn all_handles_settled(&self, asset_server: &AssetServer) -> bool {
        self.handles.values().all(|h| {
            matches!(
                asset_server.recursive_dependency_load_state(h.id()),
                RecursiveDependencyLoadState::Loaded | RecursiveDependencyLoadState::Failed(_)
            )
        })
    }

    /// Convert the resolved sets into dense bitset storage.
    pub fn freeze<S: TagSource<Id = I>>(self, source: &S) -> TagRegistry<T, I> {
        let capacity = source.capacity();
        let mut index = HashMap::with_capacity(self.resolved.len());
        let mut bitsets = Vec::with_capacity(self.resolved.len());
        for (loc, set) in self.resolved {
            index.insert(loc, bitsets.len());
            bitsets.push(BitSet::from_hash_set(&set, capacity));
        }
        TagRegistry {
            index,
            bitsets,
            _marker: PhantomData,
        }
    }
}

/// A frozen tagged registry: tag `ResourceLocation` → dense bitset of member ids.
///
/// Membership is a single bit test instead of a hash probe; the cost is one
/// `u64` word per 64 registry entries per tag, paid once at freeze.
#[derive(Resource)]
pub struct TagRegistry<T: TaggedRegistry + 'static, I: TagId = StaticId<T>> {
    index: HashMap<ResourceLocation<Arc<str>>, usize>,
    bitsets: Vec<BitSet<I>>,
    _marker: PhantomData<fn() -> T>,
}

pub type DynTagRegistry<T> = TagRegistry<T, u32>;

impl<T: TaggedRegistry + 'static, I: TagId> Default for TagRegistry<T, I> {
    fn default() -> Self {
        TagRegistry {
            index: HashMap::new(),
            bitsets: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<T: TaggedRegistry + 'static, I: TagId> Clone for TagRegistry<T, I> {
    fn clone(&self) -> Self {
        TagRegistry {
            index: self.index.clone(),
            bitsets: self.bitsets.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: TaggedRegistry + 'static, I: TagId> TagRegistry<T, I> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether `id` is a member of the given tag. Zero-alloc: uses
    /// `Borrow<str>` for lookup, then one bit test.
    pub fn contains<S: AsRef<str>>(&self, tag: &TagKey<T, S>, id: I) -> bool {
        self.index
            .get(tag.as_str())
            .is_some_and(|&slot| self.bitsets[slot].contains(id))
    }

    /// Return the bitset for a tag, or `None` if the tag has no members here.
    pub fn get<S: AsRef<str>>(&self, tag: &TagKey<T, S>) -> Option<&BitSet<I>> {
        let &slot = self.index.get(tag.as_str())?;
        Some(&self.bitsets[slot])
    }

    pub fn is_empty(&self) -> bool {
        self.bitsets.is_empty()
    }

    /// Iterate over all (tag RL, bitset) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &BitSet<I>)> {
        self.index
            .iter()
            .map(|(loc, &slot)| (loc, &self.bitsets[slot]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBlock;
    impl TaggedRegistry for TestBlock {
        const REGISTRY_PATH: &'static str = "block";
    }

    struct TestBiome;
    impl TaggedRegistry for TestBiome {
        const REGISTRY_PATH: &'static str = "worldgen/biome";
    }

    /// A source with no entries beyond a fixed id space, so loader tests can
    /// freeze without building a whole registry.
    struct IdSpace<I>(u32, PhantomData<fn() -> I>);

    impl<I: TagId> IdSpace<I> {
        fn new(capacity: u32) -> Self {
            IdSpace(capacity, PhantomData)
        }
    }

    impl<I: TagId> TagSource for IdSpace<I> {
        type Id = I;

        fn id_of(&self, _loc: &str) -> Option<I> {
            None
        }

        fn capacity(&self) -> u32 {
            self.0
        }
    }

    fn id(raw: u32) -> StaticId<TestBlock> {
        StaticId::new(raw)
    }

    fn tag(s: &'static str) -> TagKey<TestBlock> {
        TagKey::new(ResourceLocation::new_static(s))
    }

    fn biome_tag(s: &'static str) -> TagKey<TestBiome> {
        TagKey::new(ResourceLocation::new_static(s))
    }

    fn rl_arc(s: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(s).unwrap()
    }

    fn block_loader() -> TagLoader<TestBlock> {
        TagLoader::default()
    }

    fn set(ids: impl IntoIterator<Item = u32>) -> HashSet<StaticId<TestBlock>> {
        ids.into_iter().map(id).collect()
    }

    // ── Lifecycle: insert → freeze → query ──

    #[test]
    fn empty_loader_freezes_to_empty_registry() {
        let reg = block_loader().freeze(&IdSpace::new(64));
        assert!(reg.is_empty());
        assert_eq!(reg.iter().count(), 0);
    }

    #[test]
    fn insert_then_contains_after_freeze() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:mineable/pickaxe"), set([0, 5, 10]));
        let reg = loader.freeze(&IdSpace::new(128));

        assert!(!reg.is_empty());

        let tag = tag("minecraft:mineable/pickaxe");
        assert!(reg.contains(&tag, id(0)));
        assert!(reg.contains(&tag, id(5)));
        assert!(reg.contains(&tag, id(10)));
        assert!(!reg.contains(&tag, id(1)));
        assert!(!reg.contains(&tag, id(99)));
    }

    #[test]
    fn contains_unknown_tag_returns_false() {
        let reg = block_loader().freeze(&IdSpace::new(64));
        assert!(!reg.contains(&tag("minecraft:nonexistent"), id(0)));
    }

    #[test]
    fn freeze_converts_to_bitset() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:logs"), set([2, 7, 63, 64]));
        let reg = loader.freeze(&IdSpace::new(128));

        let tag = tag("minecraft:logs");
        assert!(reg.contains(&tag, id(2)));
        assert!(reg.contains(&tag, id(7)));
        assert!(reg.contains(&tag, id(63)));
        assert!(reg.contains(&tag, id(64)));
        assert!(!reg.contains(&tag, id(0)));
        assert!(!reg.contains(&tag, id(65)));
        assert!(!reg.contains(&tag, id(127)));
    }

    #[test]
    fn get_returns_bitset_after_freeze() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:sand"), set([3, 42]));
        let reg = loader.freeze(&IdSpace::new(64));

        let bs = reg.get(&tag("minecraft:sand")).expect("tag should exist");
        assert_eq!(bs.len(), 2);
        assert!(bs.contains(id(3)));
        assert!(bs.contains(id(42)));
        assert!(!bs.contains(id(0)));
    }

    #[test]
    fn get_unknown_tag_returns_none() {
        let reg = block_loader().freeze(&IdSpace::new(64));
        assert!(reg.get(&tag("minecraft:nope")).is_none());
    }

    #[test]
    fn multiple_tags_independent() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:logs"), set([1, 2]));
        loader.insert(rl_arc("minecraft:leaves"), set([2, 3]));
        let reg = loader.freeze(&IdSpace::new(64));

        let logs = tag("minecraft:logs");
        let leaves = tag("minecraft:leaves");

        assert!(reg.contains(&logs, id(1)));
        assert!(reg.contains(&logs, id(2)));
        assert!(!reg.contains(&logs, id(3)));

        assert!(!reg.contains(&leaves, id(1)));
        assert!(reg.contains(&leaves, id(2)));
        assert!(reg.contains(&leaves, id(3)));
    }

    #[test]
    fn iter_yields_all_tags() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:wool"), set([10]));
        loader.insert(rl_arc("minecraft:snow"), set([20]));
        let reg = loader.freeze(&IdSpace::new(64));

        let mut tag_names: Vec<String> = reg.iter().map(|(rl, _)| rl.as_str().to_string()).collect();
        tag_names.sort();
        assert_eq!(tag_names, vec!["minecraft:snow", "minecraft:wool"]);

        for (rl, bs) in reg.iter() {
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
    fn is_empty_false_after_freeze_with_tags() {
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:test"), set([0]));
        assert!(!loader.freeze(&IdSpace::new(64)).is_empty());
    }

    #[test]
    fn contains_matches_the_inserted_set() {
        let ids: Vec<u32> = vec![0, 1, 15, 63, 64, 100, 127, 255, 500, 999];
        let mut loader = block_loader();
        loader.insert(rl_arc("minecraft:big_tag"), set(ids.iter().copied()));
        let reg = loader.freeze(&IdSpace::new(1024));

        let tag = tag("minecraft:big_tag");
        for raw in 0..1024 {
            assert_eq!(
                reg.contains(&tag, id(raw)),
                ids.contains(&raw),
                "mismatch at id {raw}"
            );
        }
    }

    // ── The same implementation, keyed by a dynamic registry's `u32` ids ──

    #[test]
    fn dyn_loader_freezes_against_index() {
        let index = DynRegistryIndex::<TestBiome>::build(
            vec![
                rl_arc("minecraft:desert"),
                rl_arc("minecraft:forest"),
                rl_arc("minecraft:plains"),
            ]
            .into_iter(),
        );

        let mut loader = DynTagLoader::<TestBiome>::default();
        loader.insert(rl_arc("minecraft:is_forest"), HashSet::from([0u32, 2]));
        let reg = loader.freeze(&index);

        let t = biome_tag("minecraft:is_forest");
        assert!(reg.contains(&t, 0));
        assert!(reg.contains(&t, 2));
        assert!(!reg.contains(&t, 1));

        let bs = reg.get(&t).expect("tag should exist");
        assert_eq!(bs.len(), 2);

        let names: Vec<String> = reg.iter().map(|(rl, _)| rl.as_str().to_string()).collect();
        assert_eq!(names, vec!["minecraft:is_forest"]);
    }

    #[test]
    fn dyn_registry_is_empty_when_nothing_resolved() {
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        let reg = DynTagLoader::<TestBiome>::default().freeze(&index);
        assert!(reg.is_empty());
        assert_eq!(reg.iter().count(), 0);
    }

    // ── resolve_tag_file ──

    fn biome_index() -> DynRegistryIndex<TestBiome> {
        DynRegistryIndex::build(
            vec![
                rl_arc("minecraft:desert"),
                rl_arc("minecraft:forest"),
                rl_arc("minecraft:plains"),
            ]
            .into_iter(),
        )
    }

    #[test]
    fn resolve_elements() {
        let tag_file = TagFile {
            replace: false,
            values: vec![
                TagEntry::Element(rl_arc("minecraft:forest")),
                TagEntry::Element(rl_arc("minecraft:plains")),
            ],
        };

        let all_files = Assets::<TagFile>::default();
        let result = resolve_tag_file(&tag_file, &all_files, &biome_index());
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(!result.contains(&0));
    }

    #[test]
    fn resolve_optional_element_missing() {
        let index = DynRegistryIndex::<TestBiome>::build(std::iter::empty());
        let tag_file = TagFile {
            replace: false,
            values: vec![TagEntry::OptionalElement(rl_arc("minecraft:nonexistent"))],
        };

        let all_files = Assets::<TagFile>::default();
        assert!(resolve_tag_file(&tag_file, &all_files, &index).is_empty());
    }

    #[test]
    fn resolve_follows_nested_tag_references() {
        let mut all_files = Assets::<TagFile>::default();
        let leaf = all_files.add(TagFile {
            replace: false,
            values: vec![TagEntry::Element(rl_arc("minecraft:desert"))],
        });
        let middle = all_files.add(TagFile {
            replace: false,
            values: vec![
                TagEntry::Tag(leaf),
                TagEntry::Element(rl_arc("minecraft:forest")),
            ],
        });
        let root = TagFile {
            replace: false,
            values: vec![
                TagEntry::OptionalTag(middle),
                TagEntry::Element(rl_arc("minecraft:plains")),
            ],
        };

        let result = resolve_tag_file(&root, &all_files, &biome_index());
        assert_eq!(result, HashSet::from([0, 1, 2]));
    }
}
