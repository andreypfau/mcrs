use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::memory::{Dir, MemoryAssetReader};
use bevy_asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer};
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::packs::PackLayers;
use mcrs_minecraft_item::BannerPattern;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::registries::{read_packs, test_registries, world_registries};
use serde::Deserialize;

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
    let files: Vec<String> = std::fs::read_dir(assets().join("minecraft/worldgen/biome"))
        .unwrap()
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

    let report = br#"{"others":{},"registries":{"minecraft:banner_pattern":{"elements":true,"stable":false,"tags":true}}}"#;
    let world = world_registries(report).expect("the report parses");
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
