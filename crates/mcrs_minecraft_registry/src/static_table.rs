use crate::lookup::RegistryLookup;
use mcrs_minecraft_core::ResourceLocation;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
#[serde(try_from = "BTreeMap<ResourceLocation, RegistryReport>")]
pub struct StaticRegistryTable {
    registries: HashMap<Box<str>, StaticRegistryEntries>,
}

#[derive(Debug)]
pub struct StaticRegistryEntries {
    pub default: Option<ResourceLocation>,
    pub protocol_id: u32,
    by_name: HashMap<ResourceLocation, u32>,
    by_id: Vec<ResourceLocation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryReport {
    #[serde(default)]
    default: Option<ResourceLocation>,
    protocol_id: u32,
    entries: BTreeMap<ResourceLocation, EntryReport>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryReport {
    protocol_id: u32,
}

impl TryFrom<BTreeMap<ResourceLocation, RegistryReport>> for StaticRegistryTable {
    type Error = String;

    fn try_from(report: BTreeMap<ResourceLocation, RegistryReport>) -> Result<Self, String> {
        let mut registries = HashMap::with_capacity(report.len());
        for (key, registry) in report {
            let mut by_id: Vec<Option<ResourceLocation>> = vec![None; registry.entries.len()];
            let mut by_name = HashMap::with_capacity(registry.entries.len());
            for (name, entry) in registry.entries {
                let slot = by_id.get_mut(entry.protocol_id as usize).ok_or_else(|| {
                    format!(
                        "{key}: {name} has protocol_id {} out of range",
                        entry.protocol_id
                    )
                })?;
                if let Some(other) = slot.replace(name.clone()) {
                    return Err(format!(
                        "{key}: {name} and {other} share protocol_id {}",
                        entry.protocol_id
                    ));
                }
                by_name.insert(name, entry.protocol_id);
            }
            let by_id = by_id.into_iter().map(Option::unwrap).collect();
            registries.insert(
                key.path().into(),
                StaticRegistryEntries {
                    default: registry.default,
                    protocol_id: registry.protocol_id,
                    by_name,
                    by_id,
                },
            );
        }
        Ok(StaticRegistryTable { registries })
    }
}

impl StaticRegistryTable {
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::from_json(&std::fs::read(path)?).map_err(std::io::Error::other)
    }

    pub fn registry(&self, registry: &str) -> Option<&StaticRegistryEntries> {
        self.registries.get(registry)
    }

    pub fn registries(&self) -> impl Iterator<Item = (&str, &StaticRegistryEntries)> {
        self.registries.iter().map(|(k, v)| (&**k, v))
    }
}

impl StaticRegistryEntries {
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn names(&self) -> &[ResourceLocation] {
        &self.by_id
    }
}

impl RegistryLookup for StaticRegistryTable {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.registries.get(registry)?.by_name.get(name).copied()
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.registries.get(registry)?.by_id.get(id as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup::{ChainLookup, NoRegistries};

    fn table() -> StaticRegistryTable {
        StaticRegistryTable::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .unwrap()
    }

    #[test]
    fn item_round_trips() {
        let t = table();
        let stone = ResourceLocation::minecraft("stone");
        let id = t.id("item", &stone).unwrap();
        assert_eq!(t.name("item", id), Some(&stone));
        assert_eq!(t.id("item", &ResourceLocation::minecraft("air")), Some(0));
        assert_eq!(
            t.registry("item").unwrap().default,
            Some(ResourceLocation::minecraft("air"))
        );
    }

    #[test]
    fn data_component_type_matches_report() {
        let t = table();
        assert_eq!(t.registry("data_component_type").unwrap().len(), 122);
        assert_eq!(
            t.id(
                "data_component_type",
                &ResourceLocation::minecraft("cushion/color")
            ),
            Some(121)
        );
    }

    #[test]
    fn unknown_is_none() {
        let t = table();
        assert_eq!(
            t.id("minecraft:item", &ResourceLocation::minecraft("stone")),
            None
        );
        assert_eq!(
            t.id("item", &ResourceLocation::minecraft("not_a_thing")),
            None
        );
        assert_eq!(t.name("item", u32::MAX), None);
        assert_eq!(t.name("nope", 0), None);
    }

    #[test]
    fn chain_first_hit_wins() {
        let t = table();
        let chain = ChainLookup(&[&NoRegistries, &t]);
        let stone = ResourceLocation::minecraft("stone");
        assert_eq!(chain.id("item", &stone), t.id("item", &stone));
        assert_eq!(
            chain.name("item", 0),
            Some(&ResourceLocation::minecraft("air"))
        );
        assert_eq!(ChainLookup(&[&NoRegistries]).id("item", &stone), None);
    }

    #[test]
    fn rejects_duplicate_and_unknown_fields() {
        let dup = br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0},"a:b":{"protocol_id":0}}}}"#;
        assert!(StaticRegistryTable::from_json(dup).is_err());
        let gap = br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":1}}}}"#;
        assert!(StaticRegistryTable::from_json(gap).is_err());
        let extra = br#"{"minecraft:x":{"protocol_id":0,"entries":{},"bogus":1}}"#;
        assert!(StaticRegistryTable::from_json(extra).is_err());
    }
}
