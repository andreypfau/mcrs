use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item::definition::schema::ItemDefinitionFile;
use mcrs_minecraft_item::for_each_data_component;
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::{
    AttackAnimation, AttributeModifiers, BreakSound, ComponentPatch, Enchantments, Holder,
    InteractAnimation, Lore, MaxStackSize, Rarity, RepairCost, SwingAnimation, TooltipDisplay,
    UseEffects,
};
use mcrs_minecraft_registry::Registry;
use mcrs_minecraft_registry::static_report::from_report;
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::common::items;
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registries::test_registries;

fn files() -> Vec<(String, Vec<u8>)> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/").to_owned() + CORPUS_DIRECTORY;
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| (path.display().to_string(), std::fs::read(&path).unwrap()))
        .collect();
    files.sort();
    files
}

macro_rules! round_tripped_kinds {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        fn round_tripped_kinds() -> BTreeSet<DataComponentType> {
            BTreeSet::from([$(DataComponentType::$kind),*])
        }
    };
}

for_each_data_component!(round_tripped_kinds);

#[test]
fn every_file_deserialises_and_re_serialises_identically() {
    let files = files();
    assert!(files.len() > 1000, "{} files", files.len());
    test_registries().scope(|| {
        for (path, bytes) in &files {
            let file: ItemDefinitionFile =
                serde_json::from_slice(bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            let source: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let expected = &source["minecraft:item"]["components"];
            let actual = serde_json::to_value(&file.item.components).unwrap();
            assert!(
                same_shape(&actual, expected),
                "{path}\n{actual:#}\n{expected:#}"
            );
        }
    });
}

/// A float written by Java as a `float` re-emits with `f32` precision.
fn same_shape(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match (a, b) {
        (Number(x), Number(y)) if x.is_f64() || y.is_f64() => {
            x.as_f64().unwrap() as f32 == y.as_f64().unwrap() as f32
        }
        (Array(x), Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_shape(a, b))
        }
        (Object(x), Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, a)| y.get(k).is_some_and(|b| same_shape(a, b)))
        }
        _ => a == b,
    }
}

fn item_registry() -> Registry<Item> {
    from_report(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/reports/registries.json"
    )))
    .unwrap()
    .registry_of(mcrs_minecraft_item::keys::ITEM)
    .expect("the registries report has no item registry")
}

fn ids_are_dense_and_named() {
    let items = items();
    let registry = item_registry();
    assert_eq!(items.len(), files().len());
    assert_eq!(items.len(), registry.len());
    for (index, entry) in items.iter().enumerate() {
        let reported = registry.require_by_name(entry.identifier.as_str()).unwrap();
        assert_eq!(reported.index(), index, "{}", entry.identifier);
        assert_eq!(entry.id, reported, "{}", entry.identifier);
        assert_eq!(items.id_of(entry.identifier.as_str()), Some(entry.id));
        assert!(std::ptr::eq(items.get(entry.id).unwrap(), entry));
    }
    assert_eq!(items.id_of("minecraft:air"), Some(Item::Air.id()));
    assert_eq!(items.id_of("minecraft:nothing"), None);
}

fn every_prototype_kind_is_round_tripped_by_the_protocol() {
    let kinds = round_tripped_kinds();
    for entry in items().iter() {
        for value in &entry.prototype.0 {
            assert!(
                kinds.contains(&value.kind()),
                "{}: {}",
                entry.identifier,
                value.kind().location()
            );
            assert!(
                value.kind().is_persistent(),
                "{}: {}",
                entry.identifier,
                value.kind().location()
            );
        }
    }
}

fn block_placers_and_remainders_resolve() {
    let (blocks, items) = test_corpus();
    let shulker = items
        .get(items.id_of("minecraft:shulker_box").unwrap())
        .unwrap();
    assert_eq!(
        shulker.block_placer,
        Some(
            blocks
                .block("minecraft:shulker_box")
                .unwrap()
                .default_state_id
        )
    );
    assert_eq!(shulker.container_slots, Some(27));
    let hopper = items.get(items.id_of("minecraft:hopper").unwrap()).unwrap();
    assert_eq!(hopper.container_slots, Some(5));
    let bucket = items
        .get(items.id_of("minecraft:water_bucket").unwrap())
        .unwrap();
    assert_eq!(bucket.block_placer, None);
    assert_eq!(
        bucket.crafting_remainder.as_ref().unwrap().0.item.as_str(),
        "minecraft:bucket"
    );
    assert!(items.iter().filter(|e| e.block_placer.is_some()).count() > 1000);
}

fn the_plainest_item_carries_the_common_components() {
    let items = items();
    let map = &items
        .get(items.id_of("minecraft:stick").unwrap())
        .unwrap()
        .prototype;
    assert_eq!(map.get::<MaxStackSize>(), Some(&MaxStackSize(Bounded(64))));
    assert_eq!(map.get::<Lore>().unwrap().lines.0, Vec::new());
    assert_eq!(map.get::<Enchantments>(), Some(&Enchantments(vec![])));
    assert_eq!(map.get::<RepairCost>(), Some(&RepairCost(Bounded(0))));
    assert_eq!(
        map.get::<UseEffects>(),
        Some(&UseEffects {
            can_sprint: false,
            interact_vibrations: true,
            speed_multiplier: 0.2,
        })
    );
    assert_eq!(
        map.get::<AttributeModifiers>(),
        Some(&AttributeModifiers(vec![]))
    );
    assert_eq!(map.get::<Rarity>(), Some(&Rarity::Common));
    assert_eq!(
        map.get::<BreakSound>(),
        Some(&BreakSound(Holder::Reference(
            mcrs_minecraft_sound::keys::sound_event::ENTITY_ITEM_BREAK.id()
        )))
    );
    assert_eq!(
        map.get::<TooltipDisplay>(),
        Some(&TooltipDisplay::new(false, vec![]))
    );
    assert_eq!(
        map.get::<AttackAnimation>(),
        Some(&AttackAnimation(SwingAnimation::default()))
    );
    assert_eq!(
        map.get::<InteractAnimation>(),
        Some(&InteractAnimation(SwingAnimation::default()))
    );
    assert_eq!(map.diff(map), ComponentPatch::EMPTY);
}

fn a_repeated_identifier_fails_to_load() {
    let (blocks, _) = test_corpus();
    let mut files = files();
    let stick = files
        .iter()
        .position(|(path, _)| path.ends_with("/stick.json"))
        .unwrap();
    let last = files.len() - 1;
    let stick_json: serde_json::Value = serde_json::from_slice(&files[stick].1).unwrap();
    files[last] = (
        "forged/stick.json".to_owned(),
        serde_json::to_vec(&stick_json).unwrap(),
    );
    let error =
        mcrs_minecraft_world::item::definitions::from_files(files, test_registries(), blocks)
            .unwrap_err();
    assert!(
        matches!(
            &error,
            mcrs_minecraft_world::item::definitions::ItemCorpusError::DuplicateIdentifier { item, file }
                if item == "minecraft:stick" && file == "forged/stick.json"
        ),
        "{error}"
    );
}

#[test]
fn the_item_corpus_resolves_against_the_block_corpus() {
    ids_are_dense_and_named();
    every_prototype_kind_is_round_tripped_by_the_protocol();
    block_placers_and_remainders_resolve();
    the_plainest_item_carries_the_common_components();
    a_repeated_identifier_fails_to_load();
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[allow(dead_code)]
    format_version: IgnoredAny,
    #[serde(rename = "minecraft:item")]
    item: RawItem,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    description: RawDescription,
    components: BTreeMap<String, IgnoredAny>,
    #[allow(dead_code)]
    block_placer: Option<IgnoredAny>,
    #[allow(dead_code)]
    crafting_remainder: Option<IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDescription {
    identifier: String,
    protocol_id: u16,
}

fn raw_files() -> Vec<(String, RawFile)> {
    files()
        .into_iter()
        .map(|(path, bytes)| {
            let file = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            (path, file)
        })
        .collect()
}

fn assert_no_mismatches(what: &str, mismatches: Vec<String>) {
    assert!(
        mismatches.is_empty(),
        "{} {what}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

fn definition_files_are_named_after_their_identifier(files: &[(String, RawFile)]) {
    let mismatches = files
        .iter()
        .filter_map(|(path, file)| {
            let identifier = &file.item.description.identifier;
            let stem = std::path::Path::new(path).file_stem()?.to_str()?;
            (identifier.strip_prefix("minecraft:") != Some(stem))
                .then(|| format!("{path}: identifier is {identifier}"))
        })
        .collect();
    assert_no_mismatches("files not named after their identifier", mismatches);
}

fn definition_prototypes_state_no_removal_key(files: &[(String, RawFile)]) {
    let mismatches = files
        .iter()
        .flat_map(|(path, file)| {
            file.item
                .components
                .keys()
                .filter(|key| key.starts_with('!'))
                .map(move |key| format!("{path}: {key}"))
        })
        .collect();
    assert_no_mismatches("removal keys in a prototype", mismatches);
}

fn definition_protocol_ids_match_the_registries_report(files: &[(String, RawFile)]) {
    let registries = from_report(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/reports/registries.json"
    )))
    .unwrap();
    let items = registries
        .table("minecraft:item")
        .expect("the registries report has no item registry");
    let mismatches = files
        .iter()
        .filter_map(|(path, file)| {
            let description = &file.item.description;
            let registered = items.names().get(usize::from(description.protocol_id));
            (registered.map(|name| name.as_str()) != Some(description.identifier.as_str())).then(
                || {
                    format!(
                        "{path}: protocol_id {} is {registered:?} in the registries report",
                        description.protocol_id
                    )
                },
            )
        })
        .collect();
    assert_no_mismatches("items with a different protocol id", mismatches);
}

#[test]
fn the_definition_files_agree_with_their_names_and_the_registries_report() {
    let files = raw_files();
    assert!(!files.is_empty(), "no item definition was read");
    definition_files_are_named_after_their_identifier(&files);
    definition_prototypes_state_no_removal_key(&files);
    definition_protocol_ids_match_the_registries_report(&files);
}
