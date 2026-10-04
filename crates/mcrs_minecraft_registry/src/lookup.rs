use std::collections::HashMap;

use crate::set::RegistrySet;
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

// chisle: string lookups by bare path stay while the item wire codecs take names, not typed ids; they go when the codecs take typed ids.
impl RegistryLookup for RegistrySet {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.table_at_path(registry)?.number(name.as_str())
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.table_at_path(registry)?.name(id as usize)
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
    pub fn declare(&mut self, registry: &str) {
        self.by_id.entry(registry.into()).or_default();
    }

    pub fn holds(&self, registry: &str) -> bool {
        self.by_id.contains_key(registry)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistrySet;
    use crate::static_report::from_report;
    use std::sync::LazyLock;

    static REPORT: LazyLock<Vec<u8>> = LazyLock::new(|| {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .unwrap()
    });

    static JSON: LazyLock<serde_json::Value> =
        LazyLock::new(|| serde_json::from_slice(&REPORT).unwrap());

    static SET: LazyLock<RegistrySet> = LazyLock::new(|| from_report(&REPORT).unwrap());

    #[test]
    fn the_set_answers_the_legacy_lookup_as_the_report_states() {
        for registry in ["item", "sound_event", "entity_type"] {
            let entries = JSON[format!("minecraft:{registry}")]["entries"]
                .as_object()
                .unwrap();
            assert!(entries.len() > 2, "{registry}");
            let mut sampled = 0;
            for (name, entry) in entries.iter().step_by(entries.len() / 7 + 1) {
                let stated = entry["protocol_id"].as_u64().unwrap() as u32;
                let location = ResourceLocation::parse(name).unwrap();
                assert_eq!(
                    SET.id(registry, &location),
                    Some(stated),
                    "{registry} {name}"
                );
                assert_eq!(
                    SET.name(registry, stated),
                    Some(&location),
                    "{registry} {stated}"
                );
                sampled += 1;
            }
            assert!(sampled >= 5, "{registry}");
            let absent = ResourceLocation::minecraft("not_an_entry");
            assert_eq!(SET.id(registry, &absent), None);
            assert_eq!(SET.name(registry, u32::MAX), None);
        }
        let stone = ResourceLocation::minecraft("stone");
        assert_eq!(SET.id("minecraft:item", &stone), None);
        assert_eq!(SET.id("no_such_registry", &stone), None);
        assert_eq!(SET.name("no_such_registry", 0), None);
    }

    #[test]
    fn a_chain_answers_from_the_first_lookup_that_knows() {
        let stone = ResourceLocation::minecraft("stone");
        let air = ResourceLocation::minecraft("air");
        let chain = ChainLookup(&[&NoRegistries, &*SET]);
        assert_eq!(chain.id("item", &stone), SET.id("item", &stone));
        assert!(chain.id("item", &stone).is_some());
        assert_eq!(chain.name("item", 0), Some(&air));
        assert_eq!(ChainLookup(&[&NoRegistries]).id("item", &stone), None);
    }
}
