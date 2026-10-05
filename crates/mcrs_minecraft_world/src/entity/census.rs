use super::minecraft::EntityIds;
use super::villager::VillagerProfession;
use bytes::Buf;
use mcrs_minecraft_entity::VillagerType;
use mcrs_minecraft_entity::attribute;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};
use std::collections::HashMap;
use std::path::PathBuf;

const MAGIC: &[u8; 8] = b"MCREGCE0";

struct DumpAttribute {
    id: String,
    default: f64,
    min: f64,
    max: f64,
    syncable: bool,
}

struct Census {
    ids: HashMap<String, Vec<String>>,
    attributes: Vec<DumpAttribute>,
}

fn read_census() -> Census {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/entity/fixtures/registry_census.bin");
    let mut r = open_dump(&path, MAGIC);
    let ids = (0..r.get_u32_le())
        .map(|_| {
            let registry = dump_string(&mut r);
            let entries = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
            (registry, entries)
        })
        .collect();
    let attributes = (0..r.get_u32_le())
        .map(|_| DumpAttribute {
            id: dump_string(&mut r),
            default: r.get_f64_le(),
            min: r.get_f64_le(),
            max: r.get_f64_le(),
            syncable: r.get_u8() == 1,
        })
        .collect();
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    Census { ids, attributes }
}

fn serde_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn loaded_names(registry: &str) -> Vec<String> {
    crate::registries::test_registries()
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"))
        .names()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn report_set() -> RegistrySet {
    let report = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/reports/registries.json"),
    )
    .unwrap();
    from_report(&report).unwrap()
}

#[test]
fn entity_types_follow_the_registry_order() {
    let census = read_census();
    let set = report_set();
    let table = set.table("minecraft:entity_type").unwrap();
    let actual: Vec<String> = table.names().iter().map(ToString::to_string).collect();
    assert_eq!(actual, census.ids["minecraft:entity_type"]);

    let mut missing = LoadReport::new();
    let ids = EntityIds::resolve(&set, &mut missing).unwrap_or_else(|| panic!("{missing}"));
    let attributes = set.table("minecraft:attribute").unwrap();
    assert_eq!(
        attributes
            .name(ids.max_health.index())
            .map(|name| name.as_str()),
        Some(attribute::MAX_HEALTH.identifier.as_str())
    );
}

#[test]
fn every_template_entity_kind_is_a_registered_entity_type() {
    let set = report_set();
    let table = set.table("minecraft:entity_type").unwrap();
    for id in mcrs_minecraft_worldgen_feature::template::EntityKind::IDS {
        assert!(table.number(id).is_some(), "{id} is not an entity type");
    }
}

#[test]
fn villager_types_and_professions_follow_the_registry_order() {
    let census = read_census();
    let types: Vec<String> = VillagerType::ALL.iter().map(serde_name).collect();
    assert_eq!(types, census.ids["minecraft:villager_type"]);
    let professions: Vec<String> = VillagerProfession::ALL.iter().map(serde_name).collect();
    assert_eq!(professions, census.ids["minecraft:villager_profession"]);
    for (index, kind) in VillagerType::ALL.iter().enumerate() {
        assert_eq!(kind.protocol_id() as usize, index);
    }
    for (index, profession) in VillagerProfession::ALL.iter().enumerate() {
        assert_eq!(profession.protocol_id() as usize, index);
    }
}

#[test]
fn attributes_match_the_registry_entry_for_entry() {
    let census = read_census();
    assert_eq!(attribute::ALL.len(), census.attributes.len());
    let order: Vec<String> = census.attributes.iter().map(|a| a.id.clone()).collect();
    assert_eq!(order, census.ids["minecraft:attribute"]);
    let set = report_set();
    let attributes = set.table("minecraft:attribute").unwrap();
    for (index, (ours, theirs)) in attribute::ALL.iter().zip(&census.attributes).enumerate() {
        assert_eq!(ours.identifier.to_string(), theirs.id);
        assert_eq!(
            attributes.number(&theirs.id),
            Some(index as u16),
            "{} is numbered differently in the report",
            theirs.id
        );
        assert_eq!(
            ours.default.to_bits(),
            theirs.default.to_bits(),
            "{} default",
            theirs.id
        );
        assert_eq!(
            ours.min.to_bits(),
            theirs.min.to_bits(),
            "{} min",
            theirs.id
        );
        assert_eq!(
            ours.max.to_bits(),
            theirs.max.to_bits(),
            "{} max",
            theirs.id
        );
        assert_eq!(ours.syncable, theirs.syncable, "{} syncable", theirs.id);
    }
}

#[test]
fn cat_variant_assets_sort_into_the_registry_order() {
    let census = read_census();
    assert_eq!(
        loaded_names("minecraft:cat_variant"),
        census.ids["minecraft:cat_variant"]
    );
    assert_eq!(
        loaded_names("minecraft:cat_sound_variant"),
        census.ids["minecraft:cat_sound_variant"]
    );
}
