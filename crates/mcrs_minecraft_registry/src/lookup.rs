use std::collections::HashMap;

use mcrs_minecraft_core::ResourceLocation;

pub trait RegistryLookup: Sync {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32>;
    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation>;

    /// The state id of `block` with `properties`, each unspecified property
    /// taking the block's default. A property the block lacks, or a value the
    /// property lacks, is ignored the way the block state codec ignores it.
    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u32> {
        let _ = (block, properties);
        None
    }

    /// The block of state `id` and every property, or none when `id` is the
    /// block's default state, since that is the state a bare block id names.
    fn block_state(&self, id: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        let _ = id;
        None
    }
}

pub struct ChainLookup<'a>(pub &'a [&'a dyn RegistryLookup]);

impl RegistryLookup for ChainLookup<'_> {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.0.iter().find_map(|l| l.id(registry, name))
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.0.iter().find_map(|l| l.name(registry, id))
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u32> {
        self.0
            .iter()
            .find_map(|l| l.block_state_id(block, properties))
    }

    fn block_state(&self, id: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        self.0.iter().find_map(|l| l.block_state(id))
    }
}

pub struct NoRegistries;

impl RegistryLookup for NoRegistries {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u32> {
        None
    }

    fn name(&self, _: &str, _: u32) -> Option<&ResourceLocation> {
        None
    }
}

/// Name and network id of every entry, keyed by the registry's bare path so
/// the key form matches the item component registry markers.
#[derive(Default, Debug)]
pub struct LookupIndex {
    by_name: HashMap<Box<str>, HashMap<ResourceLocation, u32>>,
    by_id: HashMap<Box<str>, Vec<Option<ResourceLocation>>>,
}

impl LookupIndex {
    pub fn insert(&mut self, registry: &str, id: u32, location: Option<ResourceLocation>) {
        let by_id = self.by_id.entry(registry.into()).or_default();
        let id = id as usize;
        if by_id.len() <= id {
            by_id.resize(id + 1, None);
        }
        by_id[id] = location.clone();
        let Some(location) = location else { return };
        self.by_name
            .entry(registry.into())
            .or_default()
            .insert(location, id as u32);
    }

    pub fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.by_name.get(registry)?.get(name).copied()
    }

    pub fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.by_id.get(registry)?.get(id as usize)?.as_ref()
    }
}
