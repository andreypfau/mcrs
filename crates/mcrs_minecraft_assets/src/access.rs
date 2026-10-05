use crate::packs::VANILLA_PACK;
use crate::snapshot::RegistrySnapshot;
use bevy_asset::Asset;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::LookupIndex;
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_registry::shared::SharedResource;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct PackSource {
    pub namespace: Arc<str>,
    pub id: Arc<str>,
}

impl PackSource {
    pub fn new(namespace: &str, id: &str) -> Self {
        Self {
            namespace: Arc::from(namespace),
            id: Arc::from(id),
        }
    }

    pub fn vanilla_core() -> Self {
        Self::new("minecraft", "core")
    }
}

pub struct RegistryEntry {
    pub location: ResourceLocation<Arc<str>>,
    pub data: Option<NbtTag>,
    pub pack_source: Option<PackSource>,
}

pub struct RegistrySnapshotErased {
    key: String,
    entries: Vec<RegistryEntry>,
}

impl RegistrySnapshotErased {
    pub fn from_entries(
        key: &str,
        entries: Vec<(ResourceLocation<Arc<str>>, Option<NbtTag>)>,
        pack_source: Option<PackSource>,
    ) -> Self {
        Self {
            key: key.to_string(),
            entries: entries
                .into_iter()
                .map(|(location, data)| RegistryEntry {
                    location,
                    data,
                    pack_source: pack_source.clone(),
                })
                .collect(),
        }
    }

    pub fn from_registry_entries(key: &str, entries: Vec<RegistryEntry>) -> Self {
        Self {
            key: key.to_string(),
            entries,
        }
    }

    pub fn from_dynamic<T: Asset>(
        key: &str,
        snapshot: &RegistrySnapshot<T>,
        set: &RegistrySet,
        pack_source: Option<PackSource>,
    ) -> Self {
        let entries = snapshot
            .entries()
            .iter()
            .enumerate()
            .map(|(id, e)| RegistryEntry {
                location: e.location.clone(),
                data: Some(e.nbt.clone()),
                pack_source: pack_source
                    .clone()
                    .filter(|_| set.pack_of(key, id) == Some(VANILLA_PACK)),
            })
            .collect();
        Self {
            key: key.to_string(),
            entries,
        }
    }
}

impl RegistrySnapshotErased {
    pub fn registry_key(&self) -> &str {
        &self.key
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter_entries(&self) -> impl Iterator<Item = &RegistryEntry> {
        self.entries.iter()
    }
}

/// Shared, freeze-before-clone registry of per-world data packets.
///
/// **Invariant:** all [`register`][Self::register] calls must complete before
/// any clone is taken. Cloning bumps the inner `Arc` refcount; `register`
/// requires exclusive access via `Arc::get_mut`, which fails (panics) once
/// any other clone exists. The production wiring puts every `register` call
/// inside `OnEnter(AppState::WorldgenFreeze)` and every clone inside
/// `OnEnter(AppState::Playing)` or later. Do not clone `RegistryAccess`
/// before `WorldgenFreeze` completes.
///
/// See also the [`register`][Self::register] doc for the panic condition.
#[derive(Resource, Clone, Default)]
pub struct RegistryAccess(Arc<RegistryAccessInner>);

#[derive(Default)]
pub struct RegistryAccessInner {
    registries: Vec<RegistrySnapshotErased>,
    lookup: OnceLock<LookupIndex>,
}

fn build_lookup_index(registries: &[RegistrySnapshotErased]) -> LookupIndex {
    let mut index = LookupIndex::default();
    for registry in registries {
        let key = registry.registry_key();
        let key: Box<str> = key.split_once(':').map_or(key, |(_, path)| path).into();
        for (network_id, entry) in (0..=u16::MAX).zip(registry.iter_entries()) {
            index.insert(&key, network_id, Some(entry.location.clone()));
        }
    }
    index
}

impl RegistryLookup for RegistryAccess {
    fn id(&self, registry: &str, name: &ResourceLocation<Arc<str>>) -> Option<u16> {
        self.lookup().id(registry, name)
    }

    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation<Arc<str>>> {
        self.lookup().name(registry, id)
    }
}

impl SharedResource for RegistryAccess {
    fn shares_with(&self, other: &Self) -> bool {
        self.shares_inner_with(other)
    }
}

impl RegistryAccess {
    /// Register a registry snapshot.
    ///
    /// # Panics
    ///
    /// Panics if any clone of this `RegistryAccess` exists at the time of the
    /// call. The inner `Arc::get_mut` check enforces the freeze-before-clone
    /// invariant: once any clone has been handed to a per-dim sub-app the
    /// refcount is > 1 and mutation is forbidden.
    pub fn register(&mut self, snapshot: RegistrySnapshotErased) {
        let inner = Arc::get_mut(&mut self.0).expect(
            "RegistryAccess: registry mutation attempted after the registry was cloned; \
             mutation must complete before WorldgenFreeze — \
             after the registry was cloned all clones share the same Arc and further \
             mutation would create divergent state",
        );
        inner.registries.push(snapshot);
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegistrySnapshotErased> {
        self.0.registries.iter()
    }

    fn lookup(&self) -> &LookupIndex {
        self.0
            .lookup
            .get_or_init(|| build_lookup_index(&self.0.registries))
    }

    pub fn len(&self) -> usize {
        self.0.registries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.registries.is_empty()
    }

    /// True when `self` and `other` share the same backing `Arc`. Exposed so
    /// callers outside this module can confirm that a clone handed to a
    /// per-dimension sub-app is a refcount bump rather than a deep copy.
    pub fn shares_inner_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_location(name: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::new("minecraft", name)
    }

    #[test]
    fn lookup_resolves_names_and_ids_by_position() {
        let mut access = RegistryAccess::default();
        access.register(RegistrySnapshotErased::from_entries(
            "minecraft:item",
            vec![
                (make_location("stone"), None),
                (make_location("dirt"), None),
            ],
            None,
        ));
        assert_eq!(access.name("item", 1), Some(&make_location("dirt")));
        assert_eq!(access.name("item", 2), None);
        assert_eq!(access.id("item", &make_location("dirt")), Some(1));
        assert_eq!(access.id("block", &make_location("dirt")), None);
    }

    #[derive(bevy_asset::Asset, bevy_reflect::TypePath)]
    struct Variant;

    #[test]
    fn an_entry_from_a_non_vanilla_pack_claims_no_vanilla_pack() {
        use bevy_asset::Assets;
        use mcrs_minecraft_registry::{Pack, PackFile, WorldRegistries};

        let registry = ResourceLocation::parse("minecraft:test_variant").unwrap();
        let file = |path: &str| PackFile {
            path: path.to_owned(),
            bytes: None,
        };
        let packs = [
            Pack {
                name: "vanilla".to_owned(),
                files: vec![file("minecraft/test_variant/a.json")],
                built: Vec::new(),
            },
            Pack {
                name: "extra".to_owned(),
                files: vec![file("minecraft/test_variant/b.json")],
                built: Vec::new(),
            },
        ];
        let set = WorldRegistries::new([registry.clone()])
            .load(&RegistrySet::new(), &packs)
            .unwrap();
        let table = set.table(registry.as_str()).unwrap();

        let mut assets = Assets::<Variant>::default();
        let pairs: Vec<_> = table
            .names()
            .iter()
            .map(|name| (name.clone(), assets.add(Variant).id()))
            .collect();
        let snapshot = RegistrySnapshot::build(table, pairs, &assets, |_| {
            Ok(mcrs_minecraft_nbt::compound::NbtCompound::new().into())
        });

        let erased = RegistrySnapshotErased::from_dynamic(
            registry.as_str(),
            &snapshot,
            &set,
            Some(PackSource::vanilla_core()),
        );
        let claimed: Vec<_> = erased
            .iter_entries()
            .map(|entry| (entry.location.as_str(), entry.pack_source.is_some()))
            .collect();
        assert_eq!(claimed, [("minecraft:a", true), ("minecraft:b", false)]);
    }

    /// Verify that calling `register` after a clone exists panics with the
    /// documented message. This exercises the `Arc::get_mut().expect(...)` path
    /// that enforces the freeze-before-clone invariant.
    #[test]
    #[should_panic(expected = "after the registry was cloned")]
    fn register_after_clone_panics() {
        let mut original = RegistryAccess::default();
        let _clone = original.clone();
        // The clone holds an Arc reference; Arc::get_mut inside register now
        // returns None and the expect panics with the documented message.
        original.register(RegistrySnapshotErased::from_entries(
            "minecraft:biome",
            vec![(make_location("plains"), None)],
            None,
        ));
    }
}
