use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use mcrs_minecraft_assets::RegistryAccess;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_registry::{DynRegistryIndex, key};
use serde::Deserialize;

#[derive(Deserialize)]
struct Report {
    registries: BTreeMap<String, Flags>,
}

#[derive(Deserialize)]
struct Flags {
    elements: bool,
    stable: bool,
}

fn crate_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = crate_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn world_registries() -> BTreeSet<String> {
    let report: Report = serde_json::from_str(&read("../../assets/mcrs/reports/datapack.json"))
        .expect("the datapack report parses");
    report
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements && !flags.stable)
        .map(|(registry, _)| registry)
        .collect()
}

fn names_in_index<T: TaggedRegistry + 'static>(app: &App) -> Vec<String> {
    let index = app.world().resource::<DynRegistryIndex<T>>();
    (0..index.len())
        .map(|id| index.location(id).expect("ids are dense").to_string())
        .collect()
}

fn ids_by_registry(app: &App) -> BTreeMap<String, Vec<String>> {
    let world_registries = world_registries();
    let mut registries: BTreeMap<String, Vec<String>> = app
        .world()
        .resource::<RegistryAccess>()
        .iter()
        .filter(|snapshot| world_registries.contains(snapshot.registry_key()))
        .map(|snapshot| {
            let names = snapshot
                .iter_entries()
                .map(|entry| entry.location.to_string())
                .collect();
            (snapshot.registry_key().to_string(), names)
        })
        .collect();

    let indexes = [
        (
            "minecraft:worldgen/biome",
            names_in_index::<key::Biome>(app),
        ),
        ("minecraft:timeline", names_in_index::<Timeline>(app)),
        (
            "minecraft:worldgen/structure",
            names_in_index::<key::Structure>(app),
        ),
    ];
    for (registry, from_index) in indexes {
        match registries.get(registry) {
            Some(from_snapshot) => assert_eq!(
                from_snapshot, &from_index,
                "{registry}: the index and the snapshot number the entries differently"
            ),
            None => {
                registries.insert(registry.to_string(), from_index);
            }
        }
    }
    registries
}

fn lines(app: &App) -> Vec<String> {
    ids_by_registry(app)
        .into_iter()
        .flat_map(|(registry, names)| {
            if names.is_empty() {
                vec![registry]
            } else {
                names
                    .into_iter()
                    .enumerate()
                    .map(|(id, name)| format!("{registry} {id} {name}"))
                    .collect()
            }
        })
        .collect()
}

pub fn the_world_registry_ids_match_the_recorded_fixture(app: &App) {
    let actual = lines(app);
    for line in &actual {
        println!("world registry id: {line}");
    }

    let recorded = read("tests/fixtures/world_registry_ids.txt");
    let recorded: Vec<&str> = recorded.lines().collect();
    let first_difference = (0..actual.len().max(recorded.len()))
        .find(|&at| actual.get(at).map(String::as_str) != recorded.get(at).copied());
    if let Some(at) = first_difference {
        panic!(
            "world registry ids differ from tests/fixtures/world_registry_ids.txt at line {}: running app has {:?}, fixture has {:?}",
            at + 1,
            actual.get(at),
            recorded.get(at),
        );
    }
}
