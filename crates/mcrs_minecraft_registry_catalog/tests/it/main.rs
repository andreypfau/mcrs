use std::collections::BTreeMap;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_registry_catalog::STATIC_REGISTRIES;
use serde::Deserialize;

#[derive(Deserialize)]
struct RegistryReport {
    protocol_id: u16,
    entries: BTreeMap<String, EntryReport>,
}

#[derive(Deserialize)]
struct EntryReport {
    protocol_id: u16,
}

fn report() -> BTreeMap<String, RegistryReport> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/reports/registries.json"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn every_catalog_row_is_a_reported_registry_in_protocol_id_order() {
    let report = report();
    let ids: Vec<u16> = STATIC_REGISTRIES
        .iter()
        .map(|(location, _)| {
            report
                .get(location.as_static_str())
                .unwrap_or_else(|| panic!("{location}: in the catalog but not in the report"))
                .protocol_id
        })
        .collect();
    assert!(ids.is_sorted(), "{ids:?}");
}

#[test]
fn every_reported_entry_is_the_catalog_entry_at_its_protocol_id() {
    let report = report();
    for (location, entries) in STATIC_REGISTRIES {
        let name = location.as_static_str();
        let registry = &report[name];
        for (entry, reported) in &registry.entries {
            assert_eq!(
                entries
                    .get(usize::from(reported.protocol_id))
                    .map(|location| location.as_static_str()),
                Some(entry.as_str()),
                "{name}: {entry}"
            );
        }
        let unreported: Vec<&str> = entries
            .iter()
            .map(|location| location.as_static_str())
            .filter(|entry| !registry.entries.contains_key(*entry))
            .collect();
        assert_eq!(
            entries.len(),
            registry.entries.len(),
            "{name}: entry count; in the catalog but not in the report: {unreported:?}"
        );
    }
}

const STONE: Id<Block> = Block::Stone.id();

#[test]
fn a_built_in_id_is_built_in_a_const() {
    let report = report();
    assert_eq!(
        STONE.number(),
        report["minecraft:block"].entries["minecraft:stone"].protocol_id
    );
    assert_eq!(Block::from_id(STONE), Some(Block::Stone));
    assert_eq!(Block::Stone.as_static_str(), "minecraft:stone");
}
