use std::collections::HashMap;

use crate::set::RegistrySet;
use mcrs_minecraft_core::ResourceLocation;

pub trait RegistryLookup: Sync {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16>;
    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation>;

    /// The state id of `block` with `properties`, each unspecified property
    /// taking the block's default. A property the block lacks, or a value the
    /// property lacks, is ignored the way the block state codec ignores it.
    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u16> {
        let _ = (block, properties);
        None
    }

    /// The block of state `id` and every property, or none when `id` is the
    /// block's default state, since that is the state a bare block id names.
    fn block_state(&self, id: u16) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        let _ = id;
        None
    }

    fn registries(&self) -> Option<&RegistrySet> {
        None
    }

    /// Reads a value only for its length: every reference reads as a placeholder.
    fn walking(&self) -> bool {
        false
    }
}

pub struct ChainLookup<'a>(pub &'a [&'a dyn RegistryLookup]);

impl RegistryLookup for ChainLookup<'_> {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
        self.0.iter().find_map(|l| l.id(registry, name))
    }

    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
        self.0.iter().find_map(|l| l.name(registry, id))
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u16> {
        self.0
            .iter()
            .find_map(|l| l.block_state_id(block, properties))
    }

    fn block_state(&self, id: u16) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        self.0.iter().find_map(|l| l.block_state(id))
    }

    fn registries(&self) -> Option<&RegistrySet> {
        self.0.iter().find_map(|l| l.registries())
    }

    fn walking(&self) -> bool {
        self.0.iter().any(|l| l.walking())
    }
}

// chisle: string lookups by bare path stay while the item wire codecs take names, not typed ids; they go when the codecs take typed ids.
impl RegistryLookup for RegistrySet {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
        self.table_at_path(registry)?.number(name.as_str())
    }

    fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
        self.table_at_path(registry)?.name(usize::from(id))
    }

    fn registries(&self) -> Option<&RegistrySet> {
        Some(self)
    }
}

pub struct NoRegistries;

impl RegistryLookup for NoRegistries {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u16> {
        None
    }

    fn name(&self, _: &str, _: u16) -> Option<&ResourceLocation> {
        None
    }
}

/// Name and network id of every entry, keyed by the registry's bare path so
/// the key form matches the item component registry markers.
#[derive(Default, Debug)]
pub struct LookupIndex {
    by_name: HashMap<Box<str>, HashMap<ResourceLocation, u16>>,
    by_id: HashMap<Box<str>, Vec<Option<ResourceLocation>>>,
}

impl LookupIndex {
    pub fn declare(&mut self, registry: &str) {
        self.by_id.entry(registry.into()).or_default();
    }

    pub fn holds(&self, registry: &str) -> bool {
        self.by_id.contains_key(registry)
    }

    pub fn insert(&mut self, registry: &str, id: u16, location: Option<ResourceLocation>) {
        let by_id = self.by_id.entry(registry.into()).or_default();
        let index = usize::from(id);
        if by_id.len() <= index {
            by_id.resize(index + 1, None);
        }
        by_id[index] = location.clone();
        let Some(location) = location else { return };
        self.by_name
            .entry(registry.into())
            .or_default()
            .insert(location, id);
    }

    pub fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
        self.by_name.get(registry)?.get(name).copied()
    }

    pub fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
        self.by_id.get(registry)?.get(usize::from(id))?.as_ref()
    }
}
