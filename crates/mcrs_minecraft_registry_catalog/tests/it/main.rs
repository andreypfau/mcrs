use std::collections::{BTreeMap, BTreeSet};

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
fn the_catalog_holds_exactly_the_reported_registries_in_protocol_id_order() {
    let report = report();

    let catalog: BTreeSet<&str> = STATIC_REGISTRIES
        .iter()
        .map(|(location, _)| location.as_static_str())
        .collect();
    let reported: BTreeSet<&str> = report.keys().map(String::as_str).collect();
    assert_eq!(
        catalog.difference(&reported).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "registries in the catalog but not in the report"
    );
    assert_eq!(
        reported.difference(&catalog).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "registries in the report but not in the catalog"
    );
    assert_eq!(STATIC_REGISTRIES.len(), report.len());

    for (name, registry) in &report {
        let row = STATIC_REGISTRIES.get(usize::from(registry.protocol_id));
        assert_eq!(
            row.map(|(location, _)| location.as_static_str()),
            Some(name.as_str()),
            "{name}: row {}",
            registry.protocol_id
        );
    }
}

#[test]
fn every_reported_entry_is_the_catalog_entry_at_its_protocol_id() {
    for (name, registry) in &report() {
        let (_, entries) = STATIC_REGISTRIES
            .iter()
            .find(|(location, _)| location.as_static_str() == name)
            .unwrap_or_else(|| panic!("{name}: no catalog row"));
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
