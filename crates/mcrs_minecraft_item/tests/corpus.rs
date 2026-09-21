mod common;

use std::collections::BTreeSet;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item::definition::schema::ItemDefinitionFile;
use mcrs_minecraft_protocol::item::for_each_data_component;
use mcrs_minecraft_protocol::item::{
    AttackAnimation, AttributeModifiers, BreakSound, ComponentPatch, Enchantments, Holder,
    InteractAnimation, ItemComponentKind, Lore, MaxStackSize, Rarity, RepairCost, SwingAnimation,
    TooltipDisplay, UseEffects,
};
use mcrs_minecraft_registry::ItemId;

use common::{corpus, items};

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
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        fn round_tripped_kinds() -> BTreeSet<ItemComponentKind> {
            BTreeSet::from([$(ItemComponentKind::$ty),*])
        }
    };
}

for_each_data_component!(round_tripped_kinds);

#[test]
fn every_file_deserialises_and_re_serialises_identically() {
    let files = files();
    assert!(files.len() > 1000, "{} files", files.len());
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

#[test]
fn ids_are_dense_and_named() {
    let items = items();
    assert_eq!(items.len(), files().len());
    for (index, entry) in items.iter().enumerate() {
        assert_eq!(entry.id, ItemId(index as u16), "{}", entry.identifier);
        assert_eq!(items.id_of(entry.identifier.as_str()), Some(entry.id));
        assert!(std::ptr::eq(items.get(entry.id).unwrap(), entry));
    }
    assert_eq!(items.id_of("minecraft:air"), Some(ItemId(0)));
    assert_eq!(items.id_of("minecraft:nothing"), None);
}

#[test]
fn every_prototype_kind_is_round_tripped_by_the_protocol() {
    let kinds = round_tripped_kinds();
    for entry in items().iter() {
        for value in &entry.prototype.0 {
            assert!(
                kinds.contains(&value.kind()),
                "{}: {}",
                entry.identifier,
                value.kind()
            );
            assert!(
                value.kind().is_persistent(),
                "{}: {}",
                entry.identifier,
                value.kind()
            );
        }
    }
}

#[test]
fn block_placers_and_remainders_resolve() {
    let (blocks, items) = corpus();
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

#[test]
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
        Some(&BreakSound(Holder::reference(ResourceLocation::minecraft(
            "entity.item.break"
        ))))
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

#[test]
fn a_repeated_identifier_fails_to_load() {
    let (blocks, _) = corpus();
    let mut files = files();
    let stick = files
        .iter()
        .position(|(path, _)| path.ends_with("/stick.json"))
        .unwrap();
    let last = files.len() - 1;
    let stick_json: serde_json::Value = serde_json::from_slice(&files[stick].1).unwrap();
    let last_json: serde_json::Value = serde_json::from_slice(&files[last].1).unwrap();
    let mut forged = stick_json;
    forged["minecraft:item"]["description"]["protocol_id"] =
        last_json["minecraft:item"]["description"]["protocol_id"].clone();
    files[last] = (
        "forged/stick.json".to_owned(),
        serde_json::to_vec(&forged).unwrap(),
    );
    let error = mcrs_minecraft_item::ItemDefinitions::from_files(files, blocks).unwrap_err();
    assert!(
        matches!(
            &error,
            mcrs_minecraft_item::definition::ItemCorpusError::DuplicateIdentifier { item, file }
                if item == "minecraft:stick" && file == "forged/stick.json"
        ),
        "{error}"
    );
}
