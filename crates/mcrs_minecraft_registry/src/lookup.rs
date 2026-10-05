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
                let stated = u16::try_from(entry["protocol_id"].as_u64().unwrap()).unwrap();
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
            assert_eq!(SET.name(registry, u16::MAX), None);
        }
        let stone = ResourceLocation::minecraft("stone");
        assert_eq!(SET.id("minecraft:item", &stone), None);
        assert_eq!(SET.id("no_such_registry", &stone), None);
        assert_eq!(SET.name("no_such_registry", 0), None);
    }
}
