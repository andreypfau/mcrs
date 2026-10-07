use std::collections::HashMap;
use std::path::PathBuf;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_world::registries::world_registries;
use serde::Deserialize;

use crate::common::assets;

const REGISTRY_DATA_LOADER: &str = "src/main/java/net/minecraft/resources/RegistryDataLoader.java";
const REGISTRIES: &str = "src/main/java/net/minecraft/core/registries/Registries.java";
const REFERENCE_VERSION: &str = "src/main/resources/version.json";

#[derive(Deserialize)]
struct ReferenceVersion {
    id: String,
}

fn reference_root() -> PathBuf {
    match std::env::var_os("MCRS_REFERENCE_SOURCES") {
        Some(root) => PathBuf::from(root),
        None => PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
            .join("src/gitlab.com/andreypfau/minecraft"),
    }
}

fn read(root: &std::path::Path, relative: &str) -> String {
    let path = root.join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn is_constant_character(character: char) -> bool {
    character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
}

fn registry_paths_by_constant(registries: &str) -> HashMap<&str, &str> {
    const CALL: &str = "createRegistryKey(";
    let mut paths = HashMap::new();
    for (call, _) in registries.match_indices(CALL) {
        let argument = registries[call + CALL.len()..].trim_start();
        let Some(argument) = argument.strip_prefix('"') else {
            continue;
        };
        let path = &argument[..argument.find('"').expect("a string literal ends")];
        let declaration = registries[..call]
            .trim_end()
            .strip_suffix('=')
            .expect("a registry key constant is assigned")
            .trim_end();
        let name_start = declaration
            .rfind(|character| !is_constant_character(character))
            .map_or(0, |index| index + 1);
        paths.insert(&declaration[name_start..], path);
    }
    paths
}

fn synchronized_constants(loader: &str) -> Vec<&str> {
    const LIST: &str = "SYNCHRONIZED_REGISTRIES = List.of(";
    let list = &loader[loader
        .find(LIST)
        .expect("the loader lists the synchronized registries")
        + LIST.len()..];
    let list = &list[..list.find("\n    );").expect("the list ends")];
    list.match_indices("Registries.")
        .map(|(at, text)| {
            let rest = &list[at + text.len()..];
            let end = rest
                .find(|character| !is_constant_character(character))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect()
}

#[test]
fn the_synced_registries_and_their_order_equal_the_reference_sources() {
    let root = reference_root();
    if !root.join(REGISTRY_DATA_LOADER).is_file() {
        println!("reference sources absent at {}", root.display());
        return;
    }
    let reference: ReferenceVersion = serde_json::from_str(&read(&root, REFERENCE_VERSION))
        .unwrap_or_else(|e| panic!("{REFERENCE_VERSION} of the reference sources: {e}"));
    assert_eq!(
        reference.id, VERSION.id,
        "the reference sources are at version {} and this build targets {}",
        reference.id, VERSION.id
    );

    let loader = read(&root, REGISTRY_DATA_LOADER);
    let registries = read(&root, REGISTRIES);
    let paths = registry_paths_by_constant(&registries);
    let reference: Vec<String> = synchronized_constants(&loader)
        .into_iter()
        .map(|constant| {
            let path = paths
                .get(constant)
                .unwrap_or_else(|| panic!("Registries.{constant} is not a registry key"));
            format!("minecraft:{path}")
        })
        .collect();
    assert!(!reference.is_empty(), "no synchronized registry was read");

    let report = std::fs::read(assets().join("mcrs/reports/datapack.json")).unwrap();
    let ours: Vec<String> = world_registries(&report)
        .unwrap_or_else(|report| panic!("{report}"))
        .synced()
        .map(ToString::to_string)
        .collect();
    assert_eq!(ours, reference);
}
