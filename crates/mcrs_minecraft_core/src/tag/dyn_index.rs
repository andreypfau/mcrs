use crate::resource_location::ResourceLocation;
use crate::tag::key::TaggedRegistry;
use bevy_ecs::resource::Resource;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// A dense `ResourceLocation`-to-`u32` index for dynamic registry types.
///
/// Sorts entries alphabetically by full `namespace:path` string and assigns
/// dense 0..N indices. This deterministic ordering is reusable by
/// `RegistrySnapshot` for stable network IDs.
#[derive(Resource)]
pub struct DynRegistryIndex<T: TaggedRegistry> {
    map: HashMap<ResourceLocation<Arc<str>>, u32>,
    sorted: Vec<ResourceLocation<Arc<str>>>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: TaggedRegistry> DynRegistryIndex<T> {
    pub fn build(entries: impl Iterator<Item = ResourceLocation<Arc<str>>>) -> Self {
        let mut sorted: Vec<ResourceLocation<Arc<str>>> = entries.collect();
        sorted.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let map = sorted
            .iter()
            .enumerate()
            .map(|(i, rl)| (rl.clone(), i as u32))
            .collect();
        Self {
            map,
            sorted,
            _marker: PhantomData,
        }
    }

    pub fn get(&self, rl: &str) -> Option<u32> {
        self.map.get(rl).copied()
    }

    pub fn location(&self, id: u32) -> Option<&ResourceLocation<Arc<str>>> {
        self.sorted.get(id as usize)
    }

    pub fn len(&self) -> u32 {
        self.map.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBiome;
    impl TaggedRegistry for TestBiome {
        const REGISTRY_PATH: &'static str = "worldgen/biome";
    }

    fn rl_arc(s: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(s).unwrap()
    }

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
}
