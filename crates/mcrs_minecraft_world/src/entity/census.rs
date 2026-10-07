use bytes::Buf;
use mcrs_minecraft_entity::keys::{Attribute, VillagerProfession};
use mcrs_minecraft_registry::RegistrySet;
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
    RegistrySet::from_locations(mcrs_minecraft_registry_catalog::STATIC_REGISTRIES).unwrap()
}

#[test]
fn entity_types_follow_the_registry_order() {
    let census = read_census();
    let set = report_set();
    let table = set.table("minecraft:entity_type").unwrap();
    let actual: Vec<String> = table.names().iter().map(ToString::to_string).collect();
    assert_eq!(actual, census.ids["minecraft:entity_type"]);
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
    assert_eq!(
        mcrs_minecraft_entity::keys::VillagerType::ENTRIES
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        census.ids["minecraft:villager_type"]
    );
    assert_eq!(
        VillagerProfession::ENTRIES
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        census.ids["minecraft:villager_profession"]
    );
}

#[test]
fn attributes_match_the_registry_entry_for_entry() {
    let census = read_census();
    assert_eq!(Attribute::ALL.len(), census.attributes.len());
    let order: Vec<String> = census.attributes.iter().map(|a| a.id.clone()).collect();
    assert_eq!(order, census.ids["minecraft:attribute"]);
    for (attribute, theirs) in Attribute::ALL.iter().zip(&census.attributes) {
        let ours = attribute.definition();
        assert_eq!(attribute.location().to_string(), theirs.id);
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
