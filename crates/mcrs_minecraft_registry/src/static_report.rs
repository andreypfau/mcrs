use crate::names::NameTable;
use crate::set::RegistrySet;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _, MapAccess, Visitor};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

pub fn from_report(json: &[u8]) -> Result<RegistrySet, serde_json::Error> {
    serde_json::from_slice(json).map(|Tables(set)| set)
}

#[cfg(feature = "test-support")]
pub fn shipped_report() -> &'static RegistrySet {
    static SET: std::sync::LazyLock<RegistrySet> = std::sync::LazyLock::new(|| {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        );
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        from_report(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
    });
    &SET
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryReport {
    #[serde(default)]
    #[allow(dead_code)]
    default: Option<ResourceLocation>,
    #[allow(dead_code)]
    protocol_id: u16,
    entries: BTreeMap<ResourceLocation, EntryReport>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryReport {
    protocol_id: u16,
}

impl RegistryReport {
    fn into_table(self, registry: ResourceLocation) -> Result<NameTable, String> {
        let mut by_id: Vec<Option<ResourceLocation>> = vec![None; self.entries.len()];
        for (name, entry) in self.entries {
            let slot = by_id
                .get_mut(usize::from(entry.protocol_id))
                .ok_or_else(|| {
                    format!(
                        "registry {registry}: {name} has protocol_id {} out of range",
                        entry.protocol_id
                    )
                })?;
            if let Some(other) = slot.replace(name.clone()) {
                return Err(format!(
                    "registry {registry}: {name} and {other} share protocol_id {}",
                    entry.protocol_id
                ));
            }
        }
        NameTable::new(registry, by_id.into_iter().flatten()).map_err(|error| error.to_string())
    }
}

struct Tables(RegistrySet);

impl<'de> Deserialize<'de> for Tables {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TablesVisitor;

        impl<'de> Visitor<'de> for TablesVisitor {
            type Value = Tables;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object holding one entry per registry")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Tables, A::Error> {
                let mut tables = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(registry) = map.next_key::<ResourceLocation>()? {
                    let report: RegistryReport = map.next_value().map_err(|error| {
                        A::Error::custom(format_args!("registry {registry}: {error}"))
                    })?;
                    tables.push(Arc::new(
                        report.into_table(registry).map_err(A::Error::custom)?,
                    ));
                }
                RegistrySet::from_tables(tables)
                    .map(Tables)
                    .map_err(A::Error::custom)
            }
        }

        deserializer.deserialize_map(TablesVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::resource_location::ResourceLocation;
    use mcrs_minecraft_core::rl;
    use std::sync::LazyLock;

    struct Item;

    impl RegistryKey for Item {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:item");
    }

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

    fn stated_id(registry: &str, name: &str) -> u64 {
        JSON[registry]["entries"][name]["protocol_id"]
            .as_u64()
            .unwrap()
    }

    #[test]
    fn every_report_entry_has_its_protocol_id() {
        let registries = JSON.as_object().unwrap();
        assert_eq!(SET.tables().count(), registries.len());
        for (registry, report) in registries {
            let table = SET.table(registry).unwrap();
            let entries = report["entries"].as_object().unwrap();
            assert_eq!(table.len(), entries.len(), "{registry}");
            for (name, entry) in entries {
                let stated = entry["protocol_id"].as_u64().unwrap();
                assert_eq!(
                    table.number(name).map(u64::from),
                    Some(stated),
                    "{registry} {name}"
                );
                assert_eq!(table.name(stated as usize).unwrap().as_str(), name);
            }
        }
    }

    #[test]
    fn a_registry_without_a_key_type_resolves_by_name() {
        let arguments = SET.table("minecraft:command_argument_type").unwrap();
        assert_eq!(
            arguments.number("brigadier:float").map(u64::from),
            Some(stated_id(
                "minecraft:command_argument_type",
                "brigadier:float"
            ))
        );
        let items = SET.registry::<Item>().unwrap();
        let untyped = SET.table("minecraft:item").unwrap();
        for name in ["minecraft:air", "minecraft:stone", "minecraft:stick"] {
            assert_eq!(
                untyped.number(name),
                items.get(name).map(|id| id.number()),
                "{name}"
            );
        }
        assert!(Arc::ptr_eq(untyped, items.table()));
    }

    #[test]
    fn a_name_in_two_registries_keeps_two_ids() {
        let set = from_report(
            br#"{
                "minecraft:a": {"protocol_id": 0, "entries": {
                    "minecraft:air": {"protocol_id": 0},
                    "minecraft:stone": {"protocol_id": 1}}},
                "minecraft:b": {"protocol_id": 1, "entries": {
                    "minecraft:dirt": {"protocol_id": 1},
                    "minecraft:stone": {"protocol_id": 0}}}
            }"#,
        )
        .unwrap();
        let ids = |registry: &str| set.table(registry).unwrap().number("minecraft:stone");
        assert_eq!(ids("minecraft:a"), Some(1));
        assert_eq!(ids("minecraft:b"), Some(0));
    }

    #[test]
    fn an_empty_registry_is_present_and_empty() {
        let set = from_report(br#"{"minecraft:none": {"protocol_id": 3, "entries": {}}}"#).unwrap();
        let table = set.table("minecraft:none").unwrap();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(set.tables().count(), 1);
    }

    #[test]
    fn ids_follow_protocol_id_not_listing_order() {
        let set = from_report(
            br#"{"minecraft:x": {"protocol_id": 0, "entries": {
                "minecraft:zeta": {"protocol_id": 0},
                "minecraft:mid": {"protocol_id": 2},
                "minecraft:alpha": {"protocol_id": 1}}}}"#,
        )
        .unwrap();
        let names: Vec<&str> = set
            .table("minecraft:x")
            .unwrap()
            .names()
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(
            names,
            ["minecraft:zeta", "minecraft:alpha", "minecraft:mid"]
        );
    }

    #[test]
    fn a_malformed_report_names_the_registry() {
        let cases: [(&str, &[u8], &str); 7] = [
            (
                "a gap",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0},"a:b":{"protocol_id":2}}}}"#,
                "a:b",
            ),
            (
                "a shared id",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0},"a:b":{"protocol_id":0}}}}"#,
                "a:b",
            ),
            (
                "an id out of range",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":1}}}}"#,
                "a:a",
            ),
            (
                "an id wider than sixteen bits",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":65536}}}}"#,
                "65536",
            ),
            (
                "an unknown field of the registry",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{},"bogus":1}}"#,
                "bogus",
            ),
            (
                "an unknown field of an entry",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0,"bogus":1}}}}"#,
                "bogus",
            ),
            (
                "a registry listed twice",
                br#"{"minecraft:x":{"protocol_id":0,"entries":{}},"minecraft:x":{"protocol_id":1,"entries":{}}}"#,
                "minecraft:x",
            ),
        ];
        for (label, json, detail) in cases {
            let message = from_report(json).err().expect(label).to_string();
            assert!(message.contains("minecraft:x"), "{label}: {message}");
            assert!(message.contains(detail), "{label}: {message}");
        }
    }
}
