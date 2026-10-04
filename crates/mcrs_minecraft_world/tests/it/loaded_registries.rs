use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::memory::{Dir, MemoryAssetReader};
use bevy_asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use mcrs_minecraft_assets::RegistryAccess;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, PackLayers, VANILLA_PACK, layered_file_source};
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_item::{BannerPattern, InstrumentValue, PaintingVariantValue, SoundEvent};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{Id, Pack, PackFile, RegistrySet};
use mcrs_minecraft_world::dialog::{Action, Dialog, DialogBody, Input};
use mcrs_minecraft_world::enchantment_provider::EnchantmentProvider;
use mcrs_minecraft_world::registries::{
    read_packs, register_loaded, static_registries as build_static_registries, test_registries,
    world_registries,
};
use mcrs_minecraft_world::sulfur_cube_archetype::SulfurCubeArchetype;
use mcrs_minecraft_world::test_types::{TestEnvironment, TestInstance};
use mcrs_minecraft_world::variant::{NetworkWolfVariant, WolfVariant};
use mcrs_minecraft_world::villager_trade::VillagerTrade;
use serde::Deserialize;
use std::sync::LazyLock;

const PARSED_REPORT: &[u8] = br#"{"others":{},"registries":{
    "minecraft:banner_pattern":{"elements":true,"stable":false,"tags":true},
    "minecraft:instrument":{"elements":true,"stable":false,"tags":true},
    "minecraft:jukebox_song":{"elements":true,"stable":false,"tags":true},
    "minecraft:painting_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:trim_material":{"elements":true,"stable":false,"tags":true},
    "minecraft:trim_pattern":{"elements":true,"stable":false,"tags":true},
    "minecraft:damage_type":{"elements":true,"stable":false,"tags":true},
    "minecraft:decorated_pot_pattern":{"elements":true,"stable":false,"tags":true},
    "minecraft:block_transformer":{"elements":true,"stable":false,"tags":true},
    "minecraft:wolf_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:wolf_sound_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:pig_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:pig_sound_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:cow_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:cow_sound_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:chicken_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:chicken_sound_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:cat_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:cat_sound_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:frog_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:zombie_nautilus_variant":{"elements":true,"stable":false,"tags":true},
    "minecraft:chat_type":{"elements":true,"stable":false,"tags":true},
    "minecraft:test_environment":{"elements":true,"stable":false,"tags":true},
    "minecraft:test_instance":{"elements":true,"stable":false,"tags":true},
    "minecraft:dialog":{"elements":true,"stable":false,"tags":true},
    "minecraft:enchantment":{"elements":true,"stable":false,"tags":true},
    "minecraft:enchantment_provider":{"elements":true,"stable":false,"tags":true},
    "minecraft:sulfur_cube_archetype":{"elements":true,"stable":false,"tags":true},
    "minecraft:villager_trade":{"elements":true,"stable":false,"tags":true},
    "minecraft:trade_set":{"elements":true,"stable":false,"tags":true},
    "minecraft:world_clock":{"elements":true,"stable":false,"tags":true},
    "minecraft:timeline":{"elements":true,"stable":false,"tags":true},
    "minecraft:worldgen/block_state_provider":{"elements":true,"stable":false,"tags":false}}}"#;

static STATICS: LazyLock<RegistrySet> = LazyLock::new(|| {
    let bytes = std::fs::read(assets().join("mcrs/reports/registries.json")).unwrap();
    build_static_registries(&bytes).unwrap().0
});

fn refused_by_the_loader(registry: &str, name: &str, json: &str) -> String {
    refused_in(PARSED_REPORT, registry, name, json)
}

fn report_declaring(registries: &[&str]) -> Vec<u8> {
    let mut report: serde_json::Value = serde_json::from_slice(PARSED_REPORT).unwrap();
    for registry in registries {
        report["registries"][registry] =
            serde_json::json!({"elements": true, "stable": false, "tags": true});
    }
    serde_json::to_vec(&report).unwrap()
}

fn refused_in(report: &[u8], registry: &str, name: &str, json: &str) -> String {
    refused_among(report, &[], registry, name, json)
}

fn refused_among(
    report: &[u8],
    others: &[(&str, &str)],
    registry: &str,
    name: &str,
    json: &str,
) -> String {
    let world = world_registries(report).expect("the report parses");
    let mut files: Vec<PackFile> = others
        .iter()
        .map(|(entry, json)| PackFile {
            path: format!("minecraft/{entry}.json"),
            bytes: Some(json.as_bytes().to_vec()),
        })
        .collect();
    files.push(PackFile {
        path: format!("minecraft/{registry}/{name}.json"),
        bytes: Some(json.as_bytes().to_vec()),
    });
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files,
    }];
    world
        .load(&STATICS, &packs)
        .err()
        .unwrap_or_else(|| panic!("{registry}/{name} was accepted: {json}"))
        .to_string()
}

fn instrument(sound: &str) -> String {
    format!(
        r#"{{"sound_event":"{sound}","use_duration":7.0,"range":256.0,
        "description":{{"translate":"instrument.minecraft.ponder_goat_horn"}}}}"#
    )
}

#[derive(Deserialize)]
struct Flags {
    elements: bool,
    stable: bool,
}

#[derive(Deserialize)]
struct DatapackReport {
    registries: BTreeMap<String, Flags>,
}

fn assets() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn datapack_report() -> DatapackReport {
    let path = assets().join("mcrs/reports/datapack.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn static_registries() -> BTreeSet<String> {
    let path = assets().join("mcrs/reports/registries.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let report: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
    report.into_keys().collect()
}

#[test]
fn the_declared_registries_are_the_reports_world_registries() {
    let report = datapack_report();
    let statics = static_registries();
    let set = test_registries();

    let world: BTreeSet<String> = set
        .tables()
        .map(|table| table.registry().to_string())
        .filter(|registry| !statics.contains(registry))
        .collect();
    let declared: BTreeSet<String> = report
        .registries
        .iter()
        .filter(|(_, flags)| flags.elements && !flags.stable)
        .map(|(registry, _)| registry.clone())
        .collect();
    assert_eq!(world, declared);

    for (registry, flags) in &report.registries {
        if flags.stable {
            assert!(!world.contains(registry), "{registry} is stable");
        }
        let directory = assets()
            .join("minecraft")
            .join(registry.trim_start_matches("minecraft:"));
        if directory.is_dir() {
            assert!(
                world.contains(registry) || flags.stable,
                "{registry} has files under assets/minecraft and is neither declared nor stable"
            );
        }
    }
}

#[test]
fn every_declared_registry_has_names_from_the_loader() {
    let set = test_registries();
    for (registry, flags) in &datapack_report().registries {
        if flags.elements && !flags.stable {
            assert!(set.table(registry).is_some(), "{registry} has no table");
        }
    }

    let biomes = set.table("minecraft:worldgen/biome").expect("biome table");
    let names: BTreeSet<String> = biomes.names().iter().map(|name| name.to_string()).collect();
    let builtin = mcrs_minecraft_worldgen_builtin::paths("minecraft/worldgen/biome");
    assert!(!builtin.is_empty());
    let packs: Vec<_> = std::fs::read_dir(assets().join("mcrs/datapacks"))
        .unwrap()
        .map(|pack| pack.unwrap().path().join("minecraft"))
        .collect();
    let files: Vec<String> = std::iter::once(assets().join("minecraft"))
        .chain(packs)
        .filter_map(|root| std::fs::read_dir(root.join("worldgen/biome")).ok())
        .flatten()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            Some(format!("minecraft:{}", name.strip_suffix(".json")?))
        })
        .collect();
    assert!(!files.is_empty());
    for expected in builtin
        .iter()
        .map(|path| {
            let stem = path.strip_prefix("minecraft/worldgen/biome/").unwrap();
            format!("minecraft:{}", stem.strip_suffix(".json").unwrap())
        })
        .chain(files)
    {
        assert!(
            names.contains(&expected),
            "{expected} is missing from the biome names"
        );
    }
}

#[test]
fn the_banner_pattern_column_follows_the_name_table() {
    let set = test_registries();
    let table = set
        .table("minecraft:banner_pattern")
        .expect("banner pattern table");
    let column = set
        .column::<BannerPattern>("minecraft:banner_pattern")
        .expect("banner patterns are parsed by the loader");
    assert_eq!(column.len(), table.len());
    assert!(!column.is_empty());
    for (name, pattern) in table.names().iter().zip(column) {
        assert_eq!(&pattern.asset_id.to_string(), &name.to_string());
    }
}

#[test]
fn a_local_entry_comes_from_its_pack() {
    let set = test_registries();
    for (registry, name, pack) in [
        ("minecraft:dimension_type", "minecraft:beta", "beta"),
        ("minecraft:dimension_type", "minecraft:overworld", "vanilla"),
        ("minecraft:worldgen/world_preset", "minecraft:beta", "beta"),
        (
            "minecraft:worldgen/world_preset",
            "minecraft:normal",
            "vanilla",
        ),
    ] {
        let id = set
            .table(registry)
            .and_then(|table| table.number(name))
            .unwrap_or_else(|| panic!("{registry} has no {name}"));
        assert_eq!(set.pack_of(registry, id as usize), Some(pack), "{name}");
    }
}

#[test]
fn packs_follow_vanilla_in_name_order() {
    let pattern =
        |name: &str| format!(r#"{{"asset_id":"minecraft:{name}","translation_key":"k"}}"#);
    let root = Dir::default();
    root.insert_asset_text(Path::new("minecraft/banner_pattern/a.json"), &pattern("a"));
    root.insert_asset_text(
        Path::new("mcrs/datapacks/extra/minecraft/banner_pattern/b.json"),
        &pattern("b"),
    );

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || {
            Box::new(PackLayers::new(Box::new(MemoryAssetReader {
                root: root.clone(),
            })))
        }),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    let asset_server = app.world().resource::<AssetServer>().clone();

    let world = world_registries(PARSED_REPORT).expect("the report parses");
    let packs = read_packs(&asset_server, &world, &RegistrySet::new());

    let listed: Vec<_> = packs
        .iter()
        .map(|pack| {
            (
                pack.name.as_str(),
                pack.files
                    .iter()
                    .map(|file| (file.path.as_str(), file.bytes.as_deref()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "vanilla",
                vec![(
                    "minecraft/banner_pattern/a.json",
                    Some(pattern("a").as_bytes())
                )]
            ),
            (
                "extra",
                vec![(
                    "minecraft/banner_pattern/b.json",
                    Some(pattern("b").as_bytes())
                )]
            ),
        ]
    );

    let source = asset_server.get_source(AssetSourceId::Default).unwrap();
    let bytes = bevy_tasks::block_on(read_whole(
        source.reader(),
        Path::new("minecraft/banner_pattern/b.json"),
    ))
    .expect("the layered reader reads the pack's file at its virtual path");
    assert_eq!(bytes, pattern("b").as_bytes());
}

fn shipped_file(
    pack: &str,
    registry_path: &str,
    namespace: &str,
    path: &str,
) -> std::path::PathBuf {
    let root = if pack == VANILLA_PACK {
        assets()
    } else {
        assets().join(PACKS_ROOT).join(pack)
    };
    root.join(namespace)
        .join(registry_path)
        .join(format!("{path}.json"))
}

#[test]
fn every_shipped_file_of_a_parsed_registry_round_trips() {
    let set = test_registries();
    let datapack = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let world = world_registries(&datapack).expect("the report parses");

    let mut parsed = 0;
    for registry in world.declared().filter(|r| world.parses(r.as_str())) {
        parsed += 1;
        let table = set
            .table(registry.as_str())
            .unwrap_or_else(|| panic!("{registry} has no table"));
        assert!(!table.is_empty(), "{registry} parses and has no entries");
        for (index, name) in table.names().iter().enumerate() {
            let pack = set
                .pack_of(registry.as_str(), index)
                .unwrap_or_else(|| panic!("{registry}/{name} names no pack"));
            let file = shipped_file(pack, registry.path(), name.namespace(), name.path());
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            let encoded = world
                .encode(set, registry.as_str(), index)
                .unwrap_or_else(|| panic!("{registry}/{name} has no encoding"))
                .unwrap_or_else(|e| panic!("{registry}/{name} does not encode: {e}"));
            let from_file: serde_json::Value = serde_json::from_str(&text).unwrap();
            let from_entry: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(
                from_entry,
                from_file,
                "{registry}/{name} ({})",
                file.display()
            );
        }
    }
    assert!(parsed > 0, "the loader parses no registry");
}

#[test]
fn an_empty_object_names_the_missing_field() {
    let text = refused_by_the_loader("instrument", "silent", "{}");
    for part in ["minecraft:instrument", "minecraft:silent", "sound_event"] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn a_bare_name_takes_the_default_namespace() {
    test_registries().scope(|| {
        let bare: InstrumentValue =
            serde_json::from_str(&instrument("item.goat_horn.sound.0")).unwrap();
        let written = serde_json::to_value(&bare).unwrap();
        assert_eq!(written["sound_event"], "minecraft:item.goat_horn.sound.0");

        let upper = serde_json::from_str::<InstrumentValue>(&instrument(
            "minecraft:Item.goat_horn.sound.0",
        ));
        assert!(upper.is_err());
    });
}

#[test]
fn an_instrument_naming_an_item_as_its_sound_is_refused() {
    let text = refused_by_the_loader("instrument", "stick_horn", &instrument("minecraft:stick"));
    for part in [
        "minecraft:sound_event",
        "minecraft:stick",
        "minecraft:stick_horn",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn a_parse_without_a_scope_fails_on_another_thread() {
    let name = "\"minecraft:item.goat_horn.sound.0\"";
    test_registries().scope(|| {
        assert!(serde_json::from_str::<Id<SoundEvent>>(name).is_ok());
        let other = std::thread::scope(|threads| {
            threads
                .spawn(|| serde_json::from_str::<Id<SoundEvent>>(name).map_err(|e| e.to_string()))
                .join()
                .unwrap()
        });
        let message = other.unwrap_err();
        assert!(message.contains("minecraft:sound_event"), "{message}");
    });
}

#[test]
fn an_instrument_or_painting_the_game_refuses_fails_to_parse() {
    let instrument = |extra: &str| {
        format!(
            r#"{{
                "sound_event": "minecraft:item.goat_horn.sound.0",
                "use_duration": 7.0,
                "range": 256.0,
                "description": {{"translate": "instrument.minecraft.ponder_goat_horn"}}
                {extra}
            }}"#
        )
    };
    assert!(serde_json::from_str::<InstrumentValue>(&instrument("")).is_ok());
    assert!(serde_json::from_str::<InstrumentValue>(&instrument(r#", "volume": 1.0"#)).is_err());

    let painting =
        |width: u32| format!(r#"{{"asset_id": "minecraft:kebab", "width": {width}, "height": 1}}"#);
    assert!(serde_json::from_str::<PaintingVariantValue>(&painting(16)).is_ok());
    assert!(serde_json::from_str::<PaintingVariantValue>(&painting(17)).is_err());
}

#[test]
fn a_strict_simple_value_refuses_an_unknown_field() {
    let stone = r#"{"id":"minecraft:stone"}"#;
    for (registry, json) in [
        (
            "damage_type",
            r#"{"message_id":"x","scaling":"never","exhaustion":0.0,"bogus":1}"#.to_owned(),
        ),
        (
            "decorated_pot_pattern",
            r#"{"asset_id":"minecraft:x_pottery_pattern","bogus":1}"#.to_owned(),
        ),
        (
            "block_transformer",
            format!(r#"[{{"block_state_provider":{stone},"bogus":1}}]"#),
        ),
        (
            "chat_type",
            r#"{"bogus":1,"chat":{"translation_key":"k","parameters":[]},
                "narration":{"translation_key":"k","parameters":[]}}"#
                .to_owned(),
        ),
        (
            "test_environment",
            r#"{"bogus":1,"type":"minecraft:weather","weather":"clear"}"#.to_owned(),
        ),
        (
            "test_instance",
            r#"{"bogus":1,"type":"minecraft:block_based","environment":"minecraft:default",
                "structure":"minecraft:empty","max_ticks":1}"#
                .to_owned(),
        ),
    ] {
        let text = refused_by_the_loader(registry, "odd", &json);
        for part in [
            format!("minecraft:{registry}"),
            "minecraft:odd".to_owned(),
            format!("minecraft/{registry}/odd.json"),
            "bogus".to_owned(),
        ] {
            assert!(text.contains(&part), "{part} missing from:\n{text}");
        }
    }
}

#[test]
fn a_damage_type_with_an_unknown_scaling_fails() {
    let text = refused_by_the_loader(
        "damage_type",
        "odd",
        r#"{"message_id":"x","scaling":"sometimes","exhaustion":0.0}"#,
    );
    for part in [
        "minecraft:damage_type",
        "minecraft:odd",
        "sometimes",
        "when_caused_by_living_non_player",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

const VARIANT_REGISTRIES: [&str; 13] = [
    "minecraft:wolf_variant",
    "minecraft:wolf_sound_variant",
    "minecraft:pig_variant",
    "minecraft:pig_sound_variant",
    "minecraft:cow_variant",
    "minecraft:cow_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:chicken_sound_variant",
    "minecraft:cat_variant",
    "minecraft:cat_sound_variant",
    "minecraft:frog_variant",
    "minecraft:zombie_nautilus_variant",
    "minecraft:painting_variant",
];

#[test]
fn a_spawn_condition_naming_an_unknown_biome_tag_fails() {
    let assets = r#"{"wild":"minecraft:a","tame":"minecraft:b","angry":"minecraft:c"}"#;
    let json = format!(
        r##"{{"assets":{assets},"baby_assets":{assets},"spawn_conditions":[
            {{"condition":{{"type":"minecraft:biome","biomes":"#minecraft:no_such_tag"}},"priority":1}}]}}"##
    );
    let report = report_declaring(&["minecraft:worldgen/biome", "minecraft:worldgen/structure"]);
    let text = refused_in(&report, "wolf_variant", "odd", &json);
    for part in [
        "minecraft:wolf_variant",
        "minecraft:odd",
        "minecraft:worldgen/biome",
        "minecraft:no_such_tag",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn an_empty_variant_registry_fails_the_load() {
    let datapack = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let world = world_registries(&datapack).expect("the report parses");
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files: Vec::new(),
    }];
    let text = world
        .load(&STATICS, &packs)
        .err()
        .expect("a pack without variants is refused")
        .to_string();
    for registry in VARIANT_REGISTRIES {
        let message = format!("Registry must be non-empty: {registry}");
        assert!(text.contains(&message), "{message} missing from:\n{text}");
    }
}

#[test]
fn the_synced_wolf_variant_has_no_spawn_conditions() {
    let set = test_registries();
    let mut access = RegistryAccess::default();
    register_loaded::<WolfVariant, _>(&mut access, set, "minecraft:wolf_variant", |variant| {
        NetworkWolfVariant::from(variant)
    });
    let synced = access
        .iter()
        .find(|snapshot| snapshot.registry_key() == "minecraft:wolf_variant")
        .expect("the wolf variants are registered")
        .iter_entries()
        .find(|entry| entry.location.as_str() == "minecraft:pale")
        .and_then(|entry| entry.data.clone())
        .expect("the pale wolf is synced");
    let synced = synced.extract_compound().expect("a variant is a compound");
    assert!(synced.get("assets").is_some());
    assert!(synced.get("spawn_conditions").is_none());

    let datapack = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let world = world_registries(&datapack).expect("the report parses");
    let index = set
        .table("minecraft:wolf_variant")
        .and_then(|table| table.number("minecraft:pale"))
        .expect("the pale wolf is loaded") as usize;
    let file: serde_json::Value = serde_json::from_str(
        &world
            .encode(set, "minecraft:wolf_variant", index)
            .expect("the pale wolf has an encoding")
            .expect("the pale wolf encodes"),
    )
    .unwrap();
    assert_eq!(
        file["spawn_conditions"],
        serde_json::json!([{"priority": 0}])
    );
}

#[test]
fn a_chat_parameter_outside_the_game_set_fails() {
    let decoration = |parameter: &str| {
        format!(r#"{{"translation_key":"chat.type.text","parameters":["sender","{parameter}"]}}"#)
    };
    let chat_type = |parameter: &str| {
        format!(
            r#"{{"chat":{},"narration":{}}}"#,
            decoration(parameter),
            decoration("content")
        )
    };

    let text = refused_by_the_loader("chat_type", "odd", &chat_type("victim"));
    for part in [
        "minecraft:chat_type",
        "minecraft:odd",
        "minecraft/chat_type/odd.json",
        "victim",
        "sender",
        "target",
        "content",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let world = world_registries(PARSED_REPORT).expect("the report parses");
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files: vec![PackFile {
            path: "minecraft/chat_type/fine.json".to_owned(),
            bytes: Some(chat_type("target").into_bytes()),
        }],
    }];
    let text = world
        .load(&STATICS, &packs)
        .err()
        .map(|report| report.to_string())
        .unwrap_or_default();
    assert!(!text.contains("minecraft:chat_type"), "{text}");
}

fn registered_names(registry: &str) -> BTreeSet<String> {
    test_registries()
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"))
        .names()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(json: &str) -> serde_json::Value {
    test_registries().scope(|| {
        let value: T = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let text = serde_json::to_string(&value).unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

fn assert_samples_round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(
    type_registry: &str,
    samples: &[(&str, &str)],
) {
    let sampled: BTreeSet<String> = samples.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert_eq!(
        sampled,
        registered_names(type_registry),
        "a sample for each type {type_registry} registers, and no other"
    );
    for (name, json) in samples {
        let written = round_trip::<T>(json);
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(written, read, "{name}");
        assert_eq!(written["type"], *name, "{json}");
    }
}

#[test]
fn every_test_environment_type_round_trips() {
    assert_samples_round_trip::<TestEnvironment>(
        "minecraft:test_environment_definition_type",
        &[
            (
                "minecraft:all_of",
                r#"{"type":"minecraft:all_of","definitions":["minecraft:default",
                    {"type":"minecraft:weather","weather":"rain"}]}"#,
            ),
            (
                "minecraft:clock_time",
                r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":6000}"#,
            ),
            (
                "minecraft:difficulty",
                r#"{"type":"minecraft:difficulty","difficulty":"hard"}"#,
            ),
            (
                "minecraft:function",
                r#"{"type":"minecraft:function","setup":"minecraft:prepare","teardown":"minecraft:clean_up"}"#,
            ),
            (
                "minecraft:game_rules",
                r#"{"type":"minecraft:game_rules","rules":{"minecraft:pvp":false,
                    "minecraft:random_tick_speed":0,"minecraft:max_minecart_speed":1000}}"#,
            ),
            (
                "minecraft:timeline_attributes",
                r#"{"type":"minecraft:timeline_attributes","timelines":["minecraft:day"]}"#,
            ),
            (
                "minecraft:weather",
                r#"{"type":"minecraft:weather","weather":"thunder"}"#,
            ),
        ],
    );

    for json in [
        r#"{"type":"minecraft:function"}"#,
        r#"{"type":"minecraft:clock_time","clock":{},"time":0}"#,
        r#"{"type":"minecraft:all_of","definitions":[]}"#,
        r#"{"type":"minecraft:timeline_attributes","timelines":[{"clock":"minecraft:overworld"}]}"#,
    ] {
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(round_trip::<TestEnvironment>(json), read);
    }

    // A field that holds its default is not written.
    assert_eq!(
        round_trip::<TestEnvironment>(
            r#"{"type":"minecraft:timeline_attributes","timelines":[{"clock":"minecraft:overworld","tracks":{},"time_markers":{}}]}"#
        ),
        serde_json::json!({
            "type": "minecraft:timeline_attributes",
            "timelines": [{"clock": "minecraft:overworld"}],
        })
    );
}

#[test]
fn every_test_instance_type_round_trips() {
    assert_samples_round_trip::<TestInstance>(
        "minecraft:test_instance_type",
        &[
            (
                "minecraft:block_based",
                r#"{"type":"minecraft:block_based","environment":"minecraft:default",
                    "structure":"minecraft:empty","max_ticks":100}"#,
            ),
            (
                "minecraft:function",
                r#"{"type":"minecraft:function","function":"minecraft:always_pass",
                    "environment":{"type":"minecraft:weather","weather":"clear"},
                    "dimension":"minecraft:the_nether","structure":"minecraft:empty",
                    "max_ticks":20,"setup_ticks":2,"required":false,"rotation":"180",
                    "manual_only":true,"max_attempts":3,"required_successes":2,
                    "sky_access":true,"padding":4}"#,
            ),
        ],
    );
}

fn instance_naming(field: &str, name: &str) -> String {
    let (environment, function) = match field {
        "environment" => (name, "minecraft:always_pass"),
        _ => ("minecraft:default", name),
    };
    format!(
        r#"{{"type":"minecraft:function","function":"{function}","environment":"{environment}",
            "structure":"minecraft:empty","max_ticks":1}}"#
    )
}

fn refused_with_environment_default(instance: &str) -> String {
    let world = world_registries(PARSED_REPORT).expect("the report parses");
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files: vec![
            PackFile {
                path: "minecraft/test_environment/default.json".to_owned(),
                bytes: Some(br#"{"type":"minecraft:all_of","definitions":[]}"#.to_vec()),
            },
            PackFile {
                path: "minecraft/test_instance/odd.json".to_owned(),
                bytes: Some(instance.as_bytes().to_vec()),
            },
        ],
    }];
    world
        .load(&STATICS, &packs)
        .err()
        .map(|report| report.to_string())
        .unwrap_or_default()
}

#[test]
fn a_test_instance_naming_an_unknown_environment_fails() {
    let text =
        refused_with_environment_default(&instance_naming("environment", "minecraft:nowhere"));
    for part in [
        "minecraft:test_environment",
        "minecraft:nowhere",
        "minecraft:odd",
        "minecraft/test_instance/odd.json",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text =
        refused_with_environment_default(&instance_naming("environment", "minecraft:default"));
    assert!(!text.contains("minecraft:test_instance"), "{text}");
}

#[test]
fn a_test_instance_naming_an_unknown_function_fails() {
    let text = refused_with_environment_default(&instance_naming("function", "minecraft:nowhere"));
    for part in [
        "minecraft:test_function",
        "minecraft:nowhere",
        "minecraft:odd",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn test_values_the_game_refuses_fail_to_parse() {
    let refused = |registry: &str, json: &str, part: &str| {
        let text = refused_in(PARSED_REPORT, registry, "odd", json);
        assert!(text.contains(part), "{part} missing from:\n{text}");
    };
    let environment =
        |body: &str| format!(r#"{{"type":"minecraft:game_rules","rules":{{{body}}}}}"#);
    refused(
        "test_environment",
        &environment(r#""minecraft:pvp":1"#),
        "minecraft:pvp",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:random_tick_speed":true"#),
        "minecraft:random_tick_speed",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:max_minecart_speed":1001"#),
        "[1:1000]",
    );
    refused(
        "test_environment",
        &environment(r#""minecraft:no_such_rule":true"#),
        "minecraft:no_such_rule",
    );
    let text = refused_in(
        &report_declaring(&["minecraft:world_clock"]),
        "test_environment",
        "odd",
        r#"{"type":"minecraft:clock_time","clock":"minecraft:nowhere","time":0}"#,
    );
    for part in ["minecraft:world_clock", "minecraft:nowhere"] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
    let text = refused_among(
        PARSED_REPORT,
        &[("world_clock/overworld", "{}")],
        "test_environment",
        "odd",
        r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":-1}"#,
    );
    assert!(
        text.contains("non-negative"),
        "non-negative missing from:\n{text}"
    );
    refused(
        "test_environment",
        r#"{"type":"minecraft:lightning"}"#,
        "minecraft:lightning",
    );

    let instance = |fields: &str| {
        format!(
            r#"{{"type":"minecraft:block_based","environment":"minecraft:default",
                "structure":"minecraft:empty",{fields}}}"#
        )
    };
    for (fields, part) in [
        (r#""max_ticks":0"#, "positive"),
        (r#""max_ticks":1,"padding":129"#, "[0;128]"),
        (r#""max_ticks":1,"rotation":"sideways""#, "sideways"),
    ] {
        let text = refused_with_environment_default(&instance(fields));
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn every_integer_game_rule_is_a_registered_rule() {
    let rules = registered_names("minecraft:game_rule");
    for (name, ..) in mcrs_minecraft_world::test_types::INTEGER_GAME_RULES {
        assert!(rules.contains(name), "{name} is not a game rule");
    }
}

fn synced<T: serde::de::DeserializeOwned + serde::Serialize>(
    json: &str,
) -> mcrs_minecraft_nbt::compound::NbtCompound {
    test_registries().scope(|| {
        let value: T = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        mcrs_minecraft_nbt::to_nbt_compound(&value).unwrap()
    })
}

#[test]
fn test_values_are_synced_with_the_tags_the_game_writes() {
    let instance = synced::<TestInstance>(
        r#"{"type":"minecraft:function","function":"minecraft:always_pass",
            "environment":"minecraft:default","dimension":"minecraft:the_nether",
            "structure":"minecraft:empty","max_ticks":20,"setup_ticks":2,"required":false,
            "rotation":"180","manual_only":true,"max_attempts":3,"required_successes":2,
            "sky_access":true,"padding":4}"#,
    );
    for (field, tag) in [
        ("max_ticks", NbtTag::Int(20)),
        ("setup_ticks", NbtTag::Int(2)),
        ("max_attempts", NbtTag::Int(3)),
        ("required_successes", NbtTag::Int(2)),
        ("padding", NbtTag::Int(4)),
        ("required", NbtTag::Byte(0)),
        ("manual_only", NbtTag::Byte(1)),
        ("sky_access", NbtTag::Byte(1)),
        ("rotation", NbtTag::String("180".to_owned())),
        (
            "environment",
            NbtTag::String("minecraft:default".to_owned()),
        ),
        (
            "function",
            NbtTag::String("minecraft:always_pass".to_owned()),
        ),
    ] {
        assert_eq!(instance.get(field), Some(&tag), "{field}");
    }

    let rules = synced::<TestEnvironment>(
        r#"{"type":"minecraft:game_rules","rules":{"minecraft:pvp":false,
            "minecraft:random_tick_speed":0}}"#,
    );
    let rules = rules
        .get_compound("rules")
        .expect("the rules are a compound");
    assert_eq!(rules.get("minecraft:pvp"), Some(&NbtTag::Byte(0)));
    assert_eq!(
        rules.get("minecraft:random_tick_speed"),
        Some(&NbtTag::Int(0))
    );

    let clock = synced::<TestEnvironment>(
        r#"{"type":"minecraft:clock_time","clock":"minecraft:overworld","time":6000}"#,
    );
    assert_eq!(clock.get("time"), Some(&NbtTag::Int(6000)));
}

fn dialog_list_in(tags: &[&str], dialogs: &str) -> String {
    let world = world_registries(PARSED_REPORT).expect("the report parses");
    let mut files = vec![PackFile {
        path: "minecraft/dialog/odd.json".to_owned(),
        bytes: Some(
            format!(r#"{{"type":"minecraft:dialog_list","title":"t","dialogs":"{dialogs}"}}"#)
                .into_bytes(),
        ),
    }];
    files.extend(tags.iter().map(|tag| PackFile {
        path: format!("minecraft/tags/dialog/{tag}.json"),
        bytes: Some(br#"{"values":[]}"#.to_vec()),
    }));
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files,
    }];
    world
        .load(&STATICS, &packs)
        .err()
        .map(|report| report.to_string())
        .unwrap_or_default()
}

#[test]
fn a_dialog_list_naming_an_unknown_tag_fails() {
    let text = dialog_list_in(&["known"], "#minecraft:nowhere");
    for part in [
        "minecraft:dialog",
        "minecraft:nowhere",
        "minecraft:odd",
        "minecraft/dialog/odd.json",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    let text = dialog_list_in(&["known"], "#minecraft:known");
    assert!(!text.contains("minecraft:dialog"), "{text}");
}

const DIALOG_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:notice",
        r#"{"type":"minecraft:notice","title":{"translate":"t"},"external_title":"outer",
            "can_close_with_escape":false,"pause":false,"after_action":"none",
            "body":[{"type":"minecraft:plain_message","contents":"a"},
                {"type":"minecraft:plain_message","contents":"b","width":300}],
            "inputs":[{"type":"minecraft:text","key":"name","label":"Name"}],
            "action":{"label":"Go","tooltip":"tip","width":100,
                "action":{"type":"minecraft:run_command","command":"say hi"}}}"#,
    ),
    (
        "minecraft:confirmation",
        r#"{"type":"minecraft:confirmation","title":"Sure?",
            "body":{"type":"minecraft:plain_message","contents":"a"},
            "yes":{"label":"Yes"},
            "no":{"label":"No","action":{"type":"minecraft:show_dialog",
                "dialog":"minecraft:server_links"}}}"#,
    ),
    (
        "minecraft:multi_action",
        r#"{"type":"minecraft:multi_action","title":"Pick",
            "actions":[{"label":"A"},
                {"label":"B","action":{"type":"minecraft:change_page","page":2}}],
            "exit_action":{"label":"Back","width":200},"columns":3}"#,
    ),
    (
        "minecraft:server_links",
        r#"{"type":"minecraft:server_links","title":"Links","button_width":310,"columns":1,
            "exit_action":{"label":"Back"}}"#,
    ),
    (
        "minecraft:dialog_list",
        r#"{"type":"minecraft:dialog_list","title":"More",
            "dialogs":["minecraft:server_links","minecraft:custom_options"],
            "columns":1,"button_width":310}"#,
    ),
];

const BODY_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:item",
        r#"{"type":"minecraft:item","item":{"id":"minecraft:apple","count":2},
            "description":{"contents":"d","width":100},"show_decorations":false,
            "show_tooltip":false,"width":32,"height":64}"#,
    ),
    (
        "minecraft:plain_message",
        r#"{"type":"minecraft:plain_message","contents":{"translate":"x"},"width":300}"#,
    ),
];

const INPUT_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:boolean",
        r#"{"type":"minecraft:boolean","key":"flag","label":"F","initial":true,
            "on_true":"yes","on_false":"no"}"#,
    ),
    (
        "minecraft:number_range",
        r#"{"type":"minecraft:number_range","key":"n","width":300,"label":"N",
            "label_format":"x.y","start":0.0,"end":10.0,"initial":5.0,"step":0.5}"#,
    ),
    (
        "minecraft:single_option",
        r#"{"type":"minecraft:single_option","key":"o","width":100,
            "options":[{"id":"a","display":"A","initial":true},{"id":"b"}],
            "label":"O","label_visible":false}"#,
    ),
    (
        "minecraft:text",
        r#"{"type":"minecraft:text","key":"t","width":100,"label":"T","label_visible":false,
            "initial":"ab","max_length":10,"multiline":{"max_lines":4,"height":64}}"#,
    ),
];

const ACTION_SAMPLES: &[(&str, &str)] = &[
    (
        "minecraft:open_url",
        r#"{"type":"minecraft:open_url","url":"https://example.com"}"#,
    ),
    (
        "minecraft:run_command",
        r#"{"type":"minecraft:run_command","command":"say hi"}"#,
    ),
    (
        "minecraft:suggest_command",
        r#"{"type":"minecraft:suggest_command","command":"say hi"}"#,
    ),
    (
        "minecraft:show_dialog",
        r#"{"type":"minecraft:show_dialog","dialog":"minecraft:server_links"}"#,
    ),
    (
        "minecraft:change_page",
        r#"{"type":"minecraft:change_page","page":2}"#,
    ),
    (
        "minecraft:copy_to_clipboard",
        r#"{"type":"minecraft:copy_to_clipboard","value":"v"}"#,
    ),
    (
        "minecraft:custom",
        r#"{"type":"minecraft:custom","id":"minecraft:ping","payload":{"a":1}}"#,
    ),
    (
        "minecraft:dynamic/run_command",
        r#"{"type":"minecraft:dynamic/run_command","template":"say $(who)"}"#,
    ),
    (
        "minecraft:dynamic/custom",
        r#"{"type":"minecraft:dynamic/custom","id":"minecraft:ping","additions":{"a":"b"}}"#,
    ),
];

#[test]
fn every_dialog_type_round_trips() {
    assert_samples_round_trip::<Dialog>("minecraft:dialog_type", DIALOG_SAMPLES);
    assert_samples_round_trip::<DialogBody>("minecraft:dialog_body_type", BODY_SAMPLES);
    assert_samples_round_trip::<Input>("minecraft:input_control_type", INPUT_SAMPLES);
    assert_samples_round_trip::<Action>("minecraft:dialog_action_type", ACTION_SAMPLES);

    let read = |json: &str| round_trip::<Dialog>(json);
    assert_eq!(
        read(
            r#"{"type":"minecraft:notice","title":"t",
                "body":{"type":"minecraft:item","item":"minecraft:apple","description":"d"},
                "inputs":[{"type":"minecraft:single_option","key":"o","label":"O",
                    "options":["a","b"]}],
                "action":{"label":"ok","action":{"type":"run_command","command":"x"}}}"#
        ),
        serde_json::json!({
            "type": "minecraft:notice",
            "title": "t",
            "body": {"type": "minecraft:item", "item": {"id": "minecraft:apple"},
                "description": {"contents": "d"}},
            "inputs": [{"type": "minecraft:single_option", "key": "o", "label": "O",
                "options": [{"id": "a"}, {"id": "b"}]}],
            "action": {"label": "ok",
                "action": {"type": "minecraft:run_command", "command": "x"}},
        }),
        "alternative spellings are read as the game reads them and written in its primary form"
    );
}

#[test]
fn a_dialog_value_outside_its_range_fails() {
    let dialog = |fields: &str| format!(r#"{{"title":"t",{fields}}}"#);
    let notice = |fields: &str| dialog(&format!(r#""type":"minecraft:notice",{fields}"#));
    let input = |control: &str| {
        notice(&format!(
            r#""inputs":[{{"key":"k","label":"L",{control}}}]"#
        ))
    };
    let action = |action: &str| notice(&format!(r#""action":{{"label":"x","action":{action}}}"#));

    for (json, part) in [
        (notice(r#""action":{"label":"x","width":0}"#), "[1;1024]"),
        (notice(r#""action":{"label":"x","width":1025}"#), "[1;1024]"),
        (
            dialog(r#""type":"minecraft:server_links","columns":0"#),
            "positive",
        ),
        (
            dialog(r#""type":"minecraft:server_links","button_width":0"#),
            "[1;1024]",
        ),
        (
            input(r#""type":"minecraft:number_range","start":0.0,"end":1.0,"step":0.0"#),
            "positive",
        ),
        (
            input(r#""type":"minecraft:number_range","start":0.0,"end":1.0,"initial":2.0"#),
            "outside of range",
        ),
        (
            input(r#""type":"minecraft:text","max_length":0"#),
            "positive",
        ),
        (
            input(r#""type":"minecraft:text","initial":"abc","max_length":2"#),
            "exceeds allowed size",
        ),
        (
            input(r#""type":"minecraft:single_option","options":[]"#),
            "must have contents",
        ),
        (
            input(
                r#""type":"minecraft:single_option","options":[{"id":"a","initial":true},{"id":"b","initial":true}]"#,
            ),
            "Multiple initial values",
        ),
        (input(r#""type":"minecraft:boolean","bogus":1"#), "bogus"),
        (
            notice(r#""inputs":[{"type":"minecraft:boolean","key":"a b","label":"L"}]"#),
            "not a valid input name",
        ),
        (
            dialog(r#""type":"minecraft:multi_action","actions":[]"#),
            "must have contents",
        ),
        (notice(r#""pause":true,"after_action":"none""#), "unpause"),
        (notice(r#""bogus":1"#), "bogus"),
        (notice(r#""after_action":"later""#), "later"),
        (
            dialog(r#""type":"minecraft:lightning""#),
            "minecraft:lightning",
        ),
        (
            notice(r#""body":{"type":"minecraft:plain_message","contents":"a","width":0}"#),
            "[1;1024]",
        ),
        (
            notice(r#""body":{"type":"minecraft:item","item":"minecraft:apple","width":257}"#),
            "[1;256]",
        ),
        (
            notice(
                r#""body":{"type":"minecraft:item","item":"minecraft:apple","description":{"contents":"d","width":0}}"#,
            ),
            "did not match any variant",
        ),
        (
            action(r#"{"type":"minecraft:run_command","command":"x","bogus":1}"#),
            "bogus",
        ),
        (
            action(r#"{"type":"minecraft:open_file","path":"x"}"#),
            "open_file",
        ),
        (
            action(r#"{"type":"minecraft:change_page","page":0}"#),
            "positive",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/run_command","template":"no variables"}"#),
            "No variables in macro",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/run_command","template":"say $(who"}"#),
            "Unterminated macro variable",
        ),
        (
            action(r#"{"type":"minecraft:dynamic/custom","id":"minecraft:x","bogus":1}"#),
            "bogus",
        ),
    ] {
        let text = refused_by_the_loader("dialog", "odd", &json);
        for part in [part, "minecraft:odd"] {
            assert!(text.contains(part), "{part} missing for {json}:\n{text}");
        }
    }
}

#[test]
fn dialog_values_are_synced_with_the_tags_the_game_writes() {
    let dialog = synced::<Dialog>(
        r#"{"type":"minecraft:multi_action","title":"Pick",
            "actions":[{"label":"A","width":100,
                "action":{"type":"minecraft:change_page","page":2}}],
            "inputs":[{"type":"minecraft:number_range","key":"n","label":"N",
                "start":0.0,"end":10.0,"step":0.5}],
            "pause":false,"after_action":"none","columns":3}"#,
    );
    for (field, tag) in [
        ("columns", NbtTag::Int(3)),
        ("pause", NbtTag::Byte(0)),
        ("after_action", NbtTag::String("none".to_owned())),
    ] {
        assert_eq!(dialog.get(field), Some(&tag), "{field}");
    }
    let NbtTag::List(actions) = dialog.get("actions").expect("the actions are a list") else {
        panic!("the actions are not a list");
    };
    let NbtTag::Compound(button) = &actions[0] else {
        panic!("a button is not a compound");
    };
    assert_eq!(button.get("width"), Some(&NbtTag::Int(100)));
    let NbtTag::Compound(click) = button.get("action").expect("the button acts") else {
        panic!("an action is not a compound");
    };
    assert_eq!(click.get("page"), Some(&NbtTag::Int(2)));
    assert_eq!(
        click.get("type"),
        Some(&NbtTag::String("minecraft:change_page".to_owned()))
    );
    let NbtTag::List(inputs) = dialog.get("inputs").expect("the inputs are a list") else {
        panic!("the inputs are not a list");
    };
    let NbtTag::Compound(range) = &inputs[0] else {
        panic!("an input is not a compound");
    };
    assert_eq!(range.get("start"), Some(&NbtTag::Float(0.0)));
    assert_eq!(range.get("step"), Some(&NbtTag::Float(0.5)));
}

fn enchantment_json(supported_items: &str, effects: &str) -> String {
    format!(
        r#"{{"description":{{"translate":"enchantment.minecraft.odd"}},
        "min_cost":{{"base":1,"per_level_above_first":1}},
        "max_cost":{{"base":2,"per_level_above_first":1}},
        "anvil_cost":1,"slots":["mainhand"],"supported_items":{supported_items},
        "weight":1,"max_level":1{effects}}}"#
    )
}

fn replacing_with(block_state: &str) -> String {
    format!(
        r#","effects":{{"minecraft:location_changed":[{{"effect":{{
        "type":"minecraft:replace_disk","radius":1.0,"height":1.0,
        "block_state":{block_state}}}}}]}}"#
    )
}

#[test]
fn an_enchantment_naming_an_unknown_item_tag_fails() {
    let text = refused_by_the_loader(
        "enchantment",
        "odd",
        &enchantment_json(r##""#minecraft:nowhere""##, ""),
    );
    for part in [
        "minecraft:enchantment",
        "minecraft:odd",
        "minecraft:item",
        "minecraft:nowhere",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn a_block_state_provider_reference_is_checked() {
    let text = refused_by_the_loader(
        "enchantment",
        "odd",
        &enchantment_json(
            r#""minecraft:stick""#,
            &replacing_with(r#""minecraft:no_such_provider""#),
        ),
    );
    for part in [
        "minecraft:worldgen/block_state_provider",
        "minecraft:no_such_provider",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }

    use mcrs_minecraft_item::enchantment::effects::{BlockState, BlockStateProvider};
    let set = test_registries();
    let provider = set
        .table("minecraft:worldgen/block_state_provider")
        .and_then(|table| table.names().first())
        .expect("the data pack ships a block state provider")
        .to_string();
    set.scope(|| {
        let provider = format!("\"{provider}\"");
        for json in [
            provider.as_str(),
            r#"{"id":"minecraft:frosted_ice","properties":{"age":"0"}}"#,
            r#"{"type":"minecraft:simple","state":"minecraft:stone"}"#,
            r#"{"type":"minecraft:simple","state":{"id":"minecraft:stone"}}"#,
        ] {
            let read: BlockStateProvider =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&serde_json::to_string(&read).unwrap())
                    .unwrap(),
                serde_json::from_str::<serde_json::Value>(json).unwrap(),
                "{json}"
            );
        }
        for json in [
            r#""minecraft:stone""#,
            r#"{"id":"minecraft:stone","properties":{}}"#,
        ] {
            let read: BlockState =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            assert_eq!(serde_json::to_string(&read).unwrap(), json, "{json}");
        }
        for json in [
            r#""minecraft:no_such_block""#,
            r#"{"id":"minecraft:no_such_block"}"#,
        ] {
            let message = serde_json::from_str::<BlockState>(json)
                .expect_err(json)
                .to_string();
            assert!(message.contains("minecraft:block"), "{message}");
            assert!(message.contains("minecraft:no_such_block"), "{message}");
        }
        for json in [
            r#"{"type":"minecraft:simple","id":"minecraft:stone"}"#,
            r#"{"type":"minecraft:nowhere","state":"minecraft:stone"}"#,
            r#"{"id":"minecraft:stone","state":"minecraft:stone"}"#,
            r#"{"id":"minecraft:stone","bogus":1}"#,
        ] {
            serde_json::from_str::<BlockStateProvider>(json).expect_err(json);
        }
    });
}

fn archetype_json(attribute: &str) -> String {
    format!(
        r#"{{"attribute_modifiers":[{{"amount":-1.0,"attribute":"{attribute}",
        "id":"minecraft:odd_add_knockback_resistance","operation":"add_value"}}],
        "items":"minecraft:slime_ball",
        "knockback_modifiers":{{"horizontal_power":0.4,"vertical_power":0.1}},
        "sound_settings":{{"hit_sound":"minecraft:entity.sulfur_cube.regular.hit",
        "push_sound":"minecraft:entity.sulfur_cube.regular.push",
        "push_sound_cooldown":0.5,"push_sound_impulse_threshold":0.2}}}}"#
    )
}

#[test]
fn an_archetype_modifier_naming_an_unknown_attribute_fails() {
    let text = refused_by_the_loader(
        "sulfur_cube_archetype",
        "odd",
        &archetype_json("minecraft:no_such_attribute"),
    );
    for part in [
        "minecraft:sulfur_cube_archetype",
        "minecraft:odd",
        "minecraft:attribute",
        "minecraft:no_such_attribute",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn an_archetype_the_game_refuses_fails_to_parse() {
    let valid: serde_json::Value =
        serde_json::from_str(&archetype_json("minecraft:knockback_resistance")).unwrap();
    assert_eq!(round_trip::<SulfurCubeArchetype>(&valid.to_string()), valid);

    let mut full = valid.clone();
    full["buoyant"] = serde_json::json!(true);
    full["explosion"] = serde_json::json!({"power": 3, "causes_fire": false, "fuse": 120});
    full["contact_damage"] = serde_json::json!({
        "damage_type": "minecraft:sulfur_cube_hot",
        "amount": {"type": "minecraft:uniform", "min_inclusive": 0.5, "max_exclusive": 1.5},
        "attribute_to_source": true,
    });
    assert_eq!(round_trip::<SulfurCubeArchetype>(&full.to_string()), full);

    let refused = [
        (
            "/explosion",
            serde_json::json!({"power": -1, "causes_fire": false, "fuse": 10}),
            "non-negative",
        ),
        (
            "/explosion",
            serde_json::json!({"power": 1, "causes_fire": false, "fuse": 0}),
            "positive",
        ),
        (
            "/contact_damage",
            serde_json::json!({"damage_type": "minecraft:sulfur_cube_hot", "amount": -1.0,
                "attribute_to_source": false}),
            "too low",
        ),
        (
            "/contact_damage",
            serde_json::json!({"damage_type": "minecraft:no_such_damage", "amount": 1.0,
                "attribute_to_source": false}),
            "minecraft:no_such_damage",
        ),
        (
            "/sound_settings/hit_sound",
            serde_json::json!("minecraft:no_such_sound"),
            "minecraft:no_such_sound",
        ),
        (
            "/attribute_modifiers/0/operation",
            serde_json::json!("multiply"),
            "multiply",
        ),
        ("/bogus", serde_json::json!(1), "bogus"),
    ];
    for (pointer, value, expected) in refused {
        let mut json = valid.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        json.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), value);
        let message = test_registries().scope(|| {
            serde_json::from_value::<SulfurCubeArchetype>(json)
                .expect_err(pointer)
                .to_string()
        });
        assert!(message.contains(expected), "{pointer}: {message}");
    }
}

#[test]
fn every_enchantment_provider_type_round_trips() {
    assert_samples_round_trip::<EnchantmentProvider>(
        "minecraft:enchantment_provider_type",
        &[
            (
                "minecraft:by_cost",
                r##"{"type":"minecraft:by_cost","enchantments":"#minecraft:on_mob_spawn_equipment",
                    "cost":{"type":"minecraft:uniform","min_inclusive":5,"max_inclusive":20}}"##,
            ),
            (
                "minecraft:by_cost_with_difficulty",
                r#"{"type":"minecraft:by_cost_with_difficulty",
                    "enchantments":["minecraft:sharpness","minecraft:smite"],
                    "min_cost":5,"max_cost_span":17}"#,
            ),
            (
                "minecraft:single",
                r#"{"type":"minecraft:single","enchantment":"minecraft:silk_touch","level":1}"#,
            ),
        ],
    );

    for json in [
        r#"{"type":"minecraft:by_cost","enchantments":"minecraft:sharpness","cost":7}"#,
        r##"{"type":"minecraft:by_cost_with_difficulty",
            "enchantments":"#minecraft:on_mob_spawn_equipment","min_cost":1,"max_cost_span":0}"##,
        r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness",
            "level":{"type":"minecraft:uniform","min_inclusive":1,"max_inclusive":3}}"#,
    ] {
        let read: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(round_trip::<EnchantmentProvider>(json), read, "{json}");
    }
}

#[test]
fn a_provider_naming_an_unknown_enchantment_fails() {
    for json in [
        r#"{"type":"minecraft:single","enchantment":"minecraft:no_such_enchantment","level":1}"#,
        r#"{"type":"minecraft:by_cost","enchantments":["minecraft:no_such_enchantment"],"cost":1}"#,
        r##"{"type":"minecraft:by_cost_with_difficulty","enchantments":"#minecraft:no_such_tag",
            "min_cost":1,"max_cost_span":1}"##,
    ] {
        let text = refused_by_the_loader("enchantment_provider", "odd", json);
        for part in [
            "minecraft:enchantment",
            "minecraft:odd",
            "minecraft:no_such_",
        ] {
            assert!(text.contains(part), "{part} missing from:\n{text}");
        }
    }
}

#[test]
fn a_provider_the_game_refuses_fails_to_parse() {
    for (json, expected) in [
        (
            r#"{"type":"minecraft:by_cost_with_difficulty","enchantments":"minecraft:sharpness",
                "min_cost":0,"max_cost_span":1}"#,
            "[1;10000]",
        ),
        (
            r#"{"type":"minecraft:by_cost_with_difficulty","enchantments":"minecraft:sharpness",
                "min_cost":1,"max_cost_span":10001}"#,
            "[0;10000]",
        ),
        (
            r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness"}"#,
            "level",
        ),
        (
            r#"{"type":"minecraft:nowhere","enchantment":"minecraft:sharpness","level":1}"#,
            "minecraft:nowhere",
        ),
        (
            r#"{"type":"minecraft:single","enchantment":"minecraft:sharpness","level":1,"bogus":1}"#,
            "bogus",
        ),
    ] {
        let message = test_registries().scope(|| {
            serde_json::from_str::<EnchantmentProvider>(json)
                .expect_err(json)
                .to_string()
        });
        assert!(message.contains(expected), "{json}: {message}");
    }
}

#[test]
fn a_trade_set_naming_an_unknown_trade_tag_fails() {
    let text = refused_by_the_loader(
        "trade_set",
        "odd",
        r##"{"amount":2,"trades":"#minecraft:nowhere"}"##,
    );
    for part in [
        "minecraft:trade_set",
        "minecraft:odd",
        "minecraft:villager_trade",
        "minecraft:nowhere",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn a_trade_wanting_an_unknown_item_fails() {
    let text = refused_by_the_loader(
        "villager_trade",
        "odd",
        r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:no_such_item"}}"#,
    );
    for part in [
        "minecraft:villager_trade",
        "minecraft:odd",
        "minecraft:item",
        "minecraft:no_such_item",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

fn trade_with_modifier(modifier: &str) -> String {
    format!(
        r#"{{"gives":{{"id":"minecraft:emerald"}},"wants":{{"id":"minecraft:stick"}},
        "given_item_modifier":{modifier}}}"#
    )
}

#[test]
fn a_trade_with_an_unmodelled_loot_function_fails() {
    let text = refused_by_the_loader(
        "villager_trade",
        "odd",
        &trade_with_modifier(r#"{"type":"minecraft:set_name"}"#),
    );
    for part in [
        "minecraft:villager_trade",
        "minecraft:odd",
        "minecraft:set_name",
    ] {
        assert!(text.contains(part), "{part} missing from:\n{text}");
    }
}

#[test]
fn trade_forms_beyond_the_shipped_files_round_trip() {
    let trade = |overrides: serde_json::Value| {
        let mut trade = serde_json::json!({
            "gives": {"id": "minecraft:emerald"},
            "wants": {"id": "minecraft:stick"},
        });
        trade
            .as_object_mut()
            .unwrap()
            .extend(overrides.as_object().unwrap().clone());
        trade
    };
    let cases = [
        (trade(serde_json::json!({})), trade(serde_json::json!({}))),
        (
            trade(serde_json::json!({"gives": "minecraft:emerald", "max_uses": 4, "xp": 1})),
            trade(serde_json::json!({})),
        ),
        (
            trade(serde_json::json!({
                "max_uses": {"type": "minecraft:constant", "value": 3},
                "xp": {"type": "minecraft:uniform", "min": 1,
                    "max": {"type": "minecraft:binomial", "n": 4, "p": 0.5}},
                "reputation_discount": {"type": "minecraft:constant", "value": 0.25},
            })),
            trade(serde_json::json!({
                "max_uses": 3,
                "xp": {"type": "minecraft:uniform", "min": 1,
                    "max": {"type": "minecraft:binomial", "n": 4, "p": 0.5}},
                "reputation_discount": 0.25,
            })),
        ),
        (
            trade(serde_json::json!({"given_item_modifier": [
                {"type": "minecraft:discard"},
                {"type": "minecraft:set_potion", "id": "minecraft:water"},
            ]})),
            trade(serde_json::json!({"given_item_modifier": [
                {"type": "minecraft:discard"},
                {"type": "minecraft:set_potion", "id": "minecraft:water"},
            ]})),
        ),
        (
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:filtered",
                "condition": {"type": "minecraft:inverted",
                    "term": {"type": "minecraft:weather_check", "raining": true}},
                "item_filter": {"items": "minecraft:bow"},
                "on_pass": [{"type": "minecraft:set_random_potion"}],
                "on_fail": {"type": "minecraft:discard"},
            }})),
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:filtered",
                "condition": {"type": "minecraft:inverted",
                    "term": {"type": "minecraft:weather_check", "raining": true}},
                "item_filter": {"items": "minecraft:bow"},
                "on_pass": [{"type": "minecraft:set_random_potion"}],
                "on_fail": {"type": "minecraft:discard"},
            }})),
        ),
        (
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:exploration_map",
                "destination": "#minecraft:on_woodland_mansion_maps",
                "zoom": 2, "search_radius": 50, "skip_existing_chunks": true,
            }})),
            trade(serde_json::json!({"given_item_modifier": {
                "type": "minecraft:exploration_map",
                "destination": "#minecraft:on_woodland_mansion_maps",
            }})),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(round_trip::<VillagerTrade>(&input.to_string()), expected);
    }
}

#[test]
fn trade_values_the_game_refuses_fail_to_parse() {
    let wants =
        |id: &str| format!(r#"{{"gives":{{"id":"minecraft:emerald"}},"wants":{{"id":"{id}"}}}}"#);
    let modifier = |modifier: &str| trade_with_modifier(modifier);
    let cases = [
        (wants("minecraft:air"), "minecraft:air"),
        (
            r#"{"gives":{"id":"minecraft:emerald","count":0},"wants":{"id":"minecraft:stick"}}"#
                .to_owned(),
            "[1;99]",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},"bogus":1}"#
                .to_owned(),
            "bogus",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "max_uses":{"type":"minecraft:abs","value":1}}"#
                .to_owned(),
            "minecraft:abs",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "reputation_discount":{"type":"minecraft:uniform","min":0.0,"max":1.0}}"#
                .to_owned(),
            "minecraft:uniform",
        ),
        (
            r#"{"gives":{"id":"minecraft:emerald"},"wants":{"id":"minecraft:stick"},
                "max_uses":3000000000}"#
                .to_owned(),
            "3000000000",
        ),
        (
            modifier(r#""minecraft:some_item_modifier""#),
            "minecraft:some_item_modifier",
        ),
        (
            modifier(
                r#"{"type":"minecraft:set_stew_effect","effects":[
                {"type":"minecraft:poison","duration":5},{"type":"minecraft:poison","duration":6}]}"#,
            ),
            "duplicate mob effect",
        ),
        (
            modifier(
                r#"{"type":"minecraft:set_stew_effect","effects":[
                {"type":"minecraft:no_such_effect","duration":5}]}"#,
            ),
            "minecraft:no_such_effect",
        ),
        (
            modifier(r#"{"type":"minecraft:set_potion","id":"minecraft:no_such_potion"}"#),
            "minecraft:no_such_potion",
        ),
        (
            modifier(
                r##"{"type":"minecraft:enchant_randomly","options":"#minecraft:no_such_tag"}"##,
            ),
            "minecraft:no_such_tag",
        ),
        (
            modifier(
                r##"{"type":"minecraft:exploration_map","destination":"#minecraft:no_such_tag"}"##,
            ),
            "minecraft:no_such_tag",
        ),
        (
            modifier(r#"{"type":"minecraft:discard","bogus":1}"#),
            "bogus",
        ),
    ];
    for (json, expected) in cases {
        let message = test_registries().scope(|| {
            serde_json::from_str::<VillagerTrade>(&json)
                .expect_err(&json)
                .to_string()
        });
        assert!(message.contains(expected), "{json}: {message}");
    }
}

fn timeline_file(clock: &str, markers: &str) -> String {
    format!(r#"{{"clock":"{clock}","period_ticks":24000,"time_markers":{markers}}}"#)
}

fn load_shipped_and(timelines: &[(&str, String)]) -> Result<RegistrySet, String> {
    let datapack = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let world = world_registries(&datapack).expect("the report parses");

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        layered_file_source(&AssetPlugin::default().file_path),
    );
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
    let asset_server = app.world().resource::<AssetServer>().clone();

    let mut packs = read_packs(&asset_server, &world, &STATICS);
    packs.push(Pack {
        name: "extra".to_owned(),
        files: timelines
            .iter()
            .map(|(name, json)| PackFile {
                path: format!("test/timeline/{name}.json"),
                bytes: Some(json.clone().into_bytes()),
            })
            .collect(),
    });
    world
        .load(&STATICS, &packs)
        .map_err(|report| report.to_string())
}

#[test]
fn a_time_marker_defined_twice_for_one_clock_fails_the_load() {
    let refused = load_shipped_and(&[
        (
            "first",
            timeline_file("minecraft:overworld", r#"{"test:marker":1000}"#),
        ),
        (
            "second",
            timeline_file("minecraft:overworld", r#"{"test:marker":2000}"#),
        ),
    ])
    .err()
    .expect("a marker defined by two timelines of one clock is refused");

    let (header, line) = refused.split_once('\n').expect("a header and one line");
    assert_eq!(header, "registry load failed: 1 errors in 1 registries");
    let key = "minecraft:timeline/test:second (test/timeline/second.json): ";
    let message = line
        .strip_prefix(key)
        .unwrap_or_else(|| panic!("the line is keyed by the second timeline: {line}"));
    assert!(message.contains("test:marker"), "{message}");
    assert!(message.contains("minecraft:overworld"), "{message}");
    assert!(message.contains("more than once"), "{message}");
}

#[test]
fn a_marker_reused_on_two_clocks_loads() {
    let set = load_shipped_and(&[
        (
            "first",
            timeline_file("minecraft:overworld", r#"{"test:marker":1000}"#),
        ),
        (
            "second",
            timeline_file("minecraft:the_end", r#"{"test:marker":2000}"#),
        ),
    ])
    .unwrap_or_else(|report| panic!("the markers are on two clocks: {report}"));

    let table = set.table("minecraft:timeline").expect("the timeline table");
    let timelines = set
        .column::<Timeline>("minecraft:timeline")
        .expect("the timelines parse");
    assert_eq!(timelines.len(), table.len());
    assert!(table.number("test:first").is_some() && table.number("test:second").is_some());
}

#[test]
fn a_timeline_naming_an_unknown_clock_fails() {
    let refused = load_shipped_and(&[("lost", timeline_file("minecraft:nowhere", "{}"))])
        .err()
        .expect("a clock the registry does not hold is refused");

    assert!(refused.contains("minecraft:world_clock"), "{refused}");
    assert!(refused.contains("minecraft:nowhere"), "{refused}");
    assert!(
        refused.contains("minecraft:timeline/test:lost (test/timeline/lost.json)"),
        "{refused}"
    );
}

fn json_names(directory: &Path, prefix: &str, names: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let file = entry.file_name().into_string().unwrap();
        if entry.path().is_dir() {
            json_names(&entry.path(), &format!("{prefix}{file}/"), names);
        } else if let Some(stem) = file.strip_suffix(".json") {
            names.insert(format!("minecraft:{prefix}{stem}"));
        }
    }
}

#[test]
fn trial_spawners_have_names_and_no_values() {
    let datapack = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let world = world_registries(&datapack).expect("the report parses");
    assert!(!world.parses("minecraft:trial_spawner"));
    assert!(
        world
            .declared()
            .any(|registry| registry.as_str() == "minecraft:trial_spawner")
    );
    for loot in [
        "minecraft:loot_table",
        "minecraft:predicate",
        "minecraft:item_modifier",
    ] {
        assert!(
            !world.declared().any(|registry| registry.as_str() == loot),
            "{loot} is declared"
        );
        assert!(
            test_registries().table(loot).is_none(),
            "{loot} has a table"
        );
    }

    let mut shipped = BTreeSet::new();
    json_names(&assets().join("minecraft/trial_spawner"), "", &mut shipped);
    let table = test_registries()
        .table("minecraft:trial_spawner")
        .expect("trial spawner table");
    let named: BTreeSet<String> = table.names().iter().map(|name| name.to_string()).collect();
    assert!(!named.is_empty());
    assert_eq!(named, shipped);
}
