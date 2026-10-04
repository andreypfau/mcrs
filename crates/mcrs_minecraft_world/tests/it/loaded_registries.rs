use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::memory::{Dir, MemoryAssetReader};
use bevy_asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::{PACKS_ROOT, PackLayers, VANILLA_PACK};
use mcrs_minecraft_item::{BannerPattern, InstrumentValue, PaintingVariantValue, SoundEvent};
use mcrs_minecraft_registry::{Id, Pack, PackFile, RegistrySet};
use mcrs_minecraft_world::registries::{
    read_packs, static_registries as build_static_registries, test_registries, world_registries,
};
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
    "minecraft:block_transformer":{"elements":true,"stable":false,"tags":true}}}"#;

static STATICS: LazyLock<RegistrySet> = LazyLock::new(|| {
    let bytes = std::fs::read(assets().join("mcrs/reports/registries.json")).unwrap();
    build_static_registries(&bytes).unwrap().0
});

fn refused_by_the_loader(registry: &str, name: &str, json: &str) -> String {
    let world = world_registries(PARSED_REPORT).expect("the report parses");
    let packs = [Pack {
        name: VANILLA_PACK.to_owned(),
        files: vec![PackFile {
            path: format!("minecraft/{registry}/{name}.json"),
            bytes: Some(json.as_bytes().to_vec()),
        }],
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
