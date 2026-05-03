use crate::registry::static_registry::StaticRegistry;
use crate::resource_location::ResourceLocation;
use bevy_ecs::resource::Resource;
use mcrs_nbt::compound::NbtCompound;
use std::sync::Arc;

pub struct ErasedEntry<'a> {
    pub network_id: u32,
    pub location: &'a ResourceLocation<Arc<str>>,
    pub data: Option<&'a NbtCompound>,
}

pub trait ErasedRegistrySnapshot: Send + Sync {
    fn registry_key(&self) -> &str;
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter_entries(&self) -> Box<dyn Iterator<Item = ErasedEntry<'_>> + '_>;
}

struct ErasedOwnedEntry {
    location: ResourceLocation<Arc<str>>,
    nbt: Option<NbtCompound>,
}

pub struct RegistrySnapshotErased {
    key: String,
    entries: Vec<ErasedOwnedEntry>,
}

impl RegistrySnapshotErased {
    pub fn from_entries(
        key: &str,
        entries: Vec<(ResourceLocation<Arc<str>>, Option<NbtCompound>)>,
    ) -> Self {
        Self {
            key: key.to_string(),
            entries: entries
                .into_iter()
                .map(|(location, nbt)| ErasedOwnedEntry { location, nbt })
                .collect(),
        }
    }

    pub fn from_static<T: 'static>(
        key: &str,
        registry: &StaticRegistry<T>,
        mut serialize: impl FnMut(&ResourceLocation<Arc<str>>, &'static T) -> Option<NbtCompound>,
    ) -> Self {
        let entries = registry
            .iter()
            .map(|(_id, loc, val)| {
                let nbt = serialize(loc, val);
                ErasedOwnedEntry {
                    location: loc.clone(),
                    nbt,
                }
            })
            .collect();
        Self {
            key: key.to_string(),
            entries,
        }
    }
}

impl ErasedRegistrySnapshot for RegistrySnapshotErased {
    fn registry_key(&self) -> &str {
        &self.key
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn iter_entries(&self) -> Box<dyn Iterator<Item = ErasedEntry<'_>> + '_> {
        Box::new(self.entries.iter().enumerate().map(|(i, entry)| {
            ErasedEntry {
                network_id: i as u32,
                location: &entry.location,
                data: entry.nbt.as_ref(),
            }
        }))
    }
}

#[derive(Resource, Default)]
pub struct RegistryAccess {
    registries: Vec<Box<dyn ErasedRegistrySnapshot>>,
}

impl RegistryAccess {
    pub fn register(&mut self, snapshot: Box<dyn ErasedRegistrySnapshot>) {
        self.registries.push(snapshot);
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn ErasedRegistrySnapshot> {
        self.registries.iter().map(|b| &**b)
    }

    pub fn len(&self) -> usize {
        self.registries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.registries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_location(name: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::new("minecraft", name)
    }

    fn make_nbt(key: &str, value: &str) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_string(key, value.to_string());
        nbt
    }

    #[test]
    fn erased_snapshot_returns_correct_key_len_and_entries() {
        let erased = RegistrySnapshotErased::from_entries(
            "minecraft:worldgen/biome",
            vec![
                (
                    make_location("plains"),
                    Some(make_nbt("temperature", "0.8")),
                ),
                (
                    make_location("desert"),
                    Some(make_nbt("temperature", "2.0")),
                ),
            ],
        );

        assert_eq!(erased.registry_key(), "minecraft:worldgen/biome");
        assert_eq!(erased.len(), 2);
        assert!(!erased.is_empty());

        let entries: Vec<_> = erased.iter_entries().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].network_id, 0);
        assert_eq!(entries[0].location.as_str(), "minecraft:plains");
        assert!(entries[0].data.is_some());
        assert_eq!(entries[1].network_id, 1);
        assert_eq!(entries[1].location.as_str(), "minecraft:desert");
    }

    #[test]
    fn erased_snapshot_with_none_data_iterates_correctly() {
        let erased = RegistrySnapshotErased::from_entries(
            "minecraft:sound_event",
            vec![
                (make_location("ambient.cave"), None),
                (make_location("block.anvil.break"), None),
            ],
        );

        assert_eq!(erased.len(), 2);
        let entries: Vec<_> = erased.iter_entries().collect();
        assert!(entries[0].data.is_none());
        assert!(entries[1].data.is_none());
        assert_eq!(entries[0].location.as_str(), "minecraft:ambient.cave");
    }

    #[test]
    fn registry_access_collects_heterogeneous_snapshots() {
        let biome = RegistrySnapshotErased::from_entries(
            "minecraft:worldgen/biome",
            vec![(make_location("plains"), Some(make_nbt("t", "0.8")))],
        );
        let sound = RegistrySnapshotErased::from_entries(
            "minecraft:sound_event",
            vec![(make_location("ambient.cave"), None)],
        );

        let mut access = RegistryAccess::default();
        access.register(Box::new(biome));
        access.register(Box::new(sound));

        let keys: Vec<&str> = access.iter().map(|s| s.registry_key()).collect();
        assert_eq!(keys, &["minecraft:worldgen/biome", "minecraft:sound_event"]);
    }

    #[test]
    fn registry_access_is_empty_and_len() {
        let mut access = RegistryAccess::default();
        assert!(access.is_empty());
        assert_eq!(access.len(), 0);

        let snap = RegistrySnapshotErased::from_entries(
            "minecraft:block",
            vec![(make_location("stone"), None)],
        );
        access.register(Box::new(snap));
        assert!(!access.is_empty());
        assert_eq!(access.len(), 1);
    }

    #[test]
    fn erased_registry_snapshot_is_object_safe() {
        let erased = RegistrySnapshotErased::from_entries(
            "minecraft:item",
            vec![(make_location("diamond"), None)],
        );
        let _: Box<dyn ErasedRegistrySnapshot> = Box::new(erased);
    }
}
