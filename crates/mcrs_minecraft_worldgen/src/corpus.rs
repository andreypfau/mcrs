//! Reading the shipped asset corpus off disk, for the tests that check the
//! engine against every file the game ships rather than against a fixture.

use mcrs_minecraft_core::ResourceLocation;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub fn worldgen_dir() -> PathBuf {
    assets_dir().join("minecraft/worldgen")
}

/// Every `.json` under `dir`, recursively, in a stable order.
pub fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
}

/// The id a corpus file carries: its path under `base`, without the extension.
fn id_of(base: &Path, path: &Path) -> ResourceLocation {
    let relative = path.strip_prefix(base).expect("the file is under the base");
    let name = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    ResourceLocation::parse(&format!("minecraft:{name}")).expect("a corpus path is a valid id")
}

/// One `minecraft/worldgen` registry, parsed. Every file in the folder must
/// parse: dropping the ones that do not would let a test read "the whole corpus
/// compiles" off a corpus quietly missing the entries that broke.
pub fn registry<T: DeserializeOwned>(folder: &str) -> BTreeMap<ResourceLocation, T> {
    let base = worldgen_dir().join(folder);
    json_files(&base)
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let parsed = serde_json::from_slice::<T>(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            (id_of(&base, &path), parsed)
        })
        .collect()
}

/// One named `minecraft/worldgen` asset, which must parse.
pub fn read<T: DeserializeOwned>(folder: &str, id: &ResourceLocation) -> T {
    let path = worldgen_dir()
        .join(folder)
        .join(format!("{}.json", id.path()));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
