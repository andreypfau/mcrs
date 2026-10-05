use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use mcrs_minecraft_assets::RegistryAccess;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Registry, RegistrySet};
use mcrs_minecraft_world::registries::test_registries;

use crate::common::{declared_world_registries, loaded_names};

fn crate_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = crate_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn names_in_registry<T: RegistryKey + Send + Sync + 'static>(app: &App) -> Vec<String> {
    let registry = app.world().resource::<Registry<T>>();
    registry
        .ids()
        .map(|id| registry.name(id).expect("ids are dense").to_string())
        .collect()
}

pub fn the_running_app_numbers_world_registries_as_the_loader_does(app: &App) {
    let set = app.world().resource::<RegistrySet>();
    let world_registries = declared_world_registries();

    let mut snapshots = 0;
    for snapshot in app
        .world()
        .resource::<RegistryAccess>()
        .iter()
        .filter(|snapshot| world_registries.contains(snapshot.registry_key()))
    {
        let registry = snapshot.registry_key();
        let numbered: Vec<String> = snapshot
            .iter_entries()
            .map(|entry| entry.location.to_string())
            .collect();
        assert_eq!(
            numbered,
            loaded_names(set, registry),
            "{registry}: the snapshot numbers the entries differently from the loader"
        );
        snapshots += 1;
    }
    assert!(snapshots > 0, "no world registry snapshot was compared");

    let indexes = [
        (
            "minecraft:worldgen/biome",
            names_in_registry::<keys::Biome>(app),
        ),
        (
            "minecraft:worldgen/structure",
            names_in_registry::<keys::Structure>(app),
        ),
        (
            "minecraft:timeline",
            names_in_registry::<keys::Timeline>(app),
        ),
    ];
    for (registry, numbered) in indexes {
        assert_eq!(
            numbered,
            loaded_names(set, registry),
            "{registry}: the index numbers the entries differently from the loader"
        );
    }
}

fn recorded_by_registry(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut registries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split(' ');
        let registry = parts.next().expect("a line names its registry");
        let names = registries.entry(registry.to_string()).or_default();
        if let (Some(_id), Some(name)) = (parts.next(), parts.next()) {
            names.push(name.to_string());
        }
    }
    registries
}

#[test]
fn the_loader_keeps_every_recorded_world_registry_id() {
    let set = test_registries();
    let changed: BTreeSet<String> =
        std::fs::read_to_string(crate_path("tests/fixtures/world_registry_id_changes.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();

    for (registry, recorded) in recorded_by_registry(&read("tests/fixtures/world_registry_ids.txt"))
    {
        let loaded = loaded_names(set, &registry);
        if changed.contains(&registry) {
            assert_ne!(
                loaded, recorded,
                "{registry} is listed as renumbered but keeps its recorded order"
            );
            let loaded: BTreeSet<_> = loaded.into_iter().collect();
            let recorded: BTreeSet<_> = recorded.into_iter().collect();
            assert_eq!(loaded, recorded, "{registry}: the set of names changed");
        } else {
            assert_eq!(
                loaded, recorded,
                "{registry}: the loader numbers the entries differently from the recorded ids"
            );
        }
    }
}

#[test]
fn a_world_registry_without_files_is_empty() {
    let table = test_registries()
        .table("minecraft:dimension")
        .expect("minecraft:dimension is a loaded registry");
    assert!(table.is_empty(), "{} names", table.len());
}
