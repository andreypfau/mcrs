use super::villager::{VillagerProfession, VillagerType};
use super::{attribute, minecraft};
use crate::data_pack::registry_files::{FILES_CAT_SOUND_VARIANT, FILES_CAT_VARIANT};
use bytes::Buf;
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

fn asset_ids(files: &[&str], folder: &str) -> Vec<String> {
    files
        .iter()
        .map(|file| {
            let name = file
                .strip_prefix(folder)
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(|rest| rest.strip_suffix(".json"))
                .unwrap_or_else(|| panic!("{file} is not under {folder}"));
            format!("minecraft:{name}")
        })
        .collect()
}

fn entity_type_registry() -> mcrs_minecraft_registry::StaticRegistry<super::EntityType> {
    let table = mcrs_minecraft_registry::StaticRegistryTable::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/reports/registries.json"),
    )
    .unwrap();
    let mut registry = mcrs_minecraft_registry::StaticRegistry::new();
    minecraft::register_all_entity_types(&mut registry, &table);
    registry
}

#[test]
fn entity_types_follow_the_registry_order() {
    let census = read_census();
    let expected = &census.ids["minecraft:entity_type"];
    let registry = entity_type_registry();
    let actual: Vec<String> = registry
        .iter()
        .map(|(_, _, t)| t.identifier.to_string())
        .collect();
    assert_eq!(actual, *expected);
    for (id, _, entity_type) in registry.iter() {
        assert_eq!(
            entity_type.protocol_id,
            id.raw(),
            "{}",
            entity_type.identifier
        );
    }
    assert_eq!(minecraft::PLAYER.protocol_id, 159);
    assert_eq!(minecraft::PRIMED_TNT.protocol_id, 136);
    assert_eq!(minecraft::ITEM.protocol_id, 72);
}

#[test]
fn every_template_entity_kind_is_a_registered_entity_type() {
    let registry = entity_type_registry();
    for id in mcrs_minecraft_worldgen_feature::template::EntityKind::IDS {
        assert!(registry.id_of(id).is_some(), "{id} is not an entity type");
    }
}

#[test]
fn items_carry_their_registry_index() {
    let census = read_census();
    let expected = &census.ids["minecraft:item"];
    let mut app = bevy_app::App::new();
    app.add_plugins(bevy_app::TaskPoolPlugin::default());
    app.add_plugins(bevy_asset::AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    let asset_server = app.world().resource::<bevy_asset::AssetServer>().clone();
    let (blocks, _) = mcrs_minecraft_block::definition::load_block_definitions(&asset_server)
        .expect("the block corpus loads");
    let items = mcrs_minecraft_item::load_item_definitions(&asset_server, &blocks)
        .expect("the item corpus loads");
    let actual: Vec<String> = items
        .iter()
        .map(|item| item.identifier.to_string())
        .collect();
    assert_eq!(actual, *expected);
    for (index, item) in items.iter().enumerate() {
        assert_eq!(item.id.0 as usize, index, "{}", item.identifier);
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
    for (index, (ours, theirs)) in attribute::ALL.iter().zip(&census.attributes).enumerate() {
        assert_eq!(ours.identifier.to_string(), theirs.id);
        assert_eq!(ours.protocol_id as usize, index, "{}", theirs.id);
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
        asset_ids(FILES_CAT_VARIANT, "minecraft/cat_variant"),
        census.ids["minecraft:cat_variant"]
    );
    assert_eq!(
        asset_ids(FILES_CAT_SOUND_VARIANT, "minecraft/cat_sound_variant"),
        census.ids["minecraft:cat_sound_variant"]
    );
}
