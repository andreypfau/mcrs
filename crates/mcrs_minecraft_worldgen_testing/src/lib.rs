#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "test support reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_types,
    reason = "test support re-encodes a value into JSON for comparison"
)]

use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_builtin as builtin;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub use mcrs_minecraft_fixtures::vanilla_dir;

pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub fn corpus_path(relative: &str) -> PathBuf {
    match Path::new(relative).strip_prefix("minecraft").ok() {
        Some(below) => vanilla_dir().join(below),
        None => assets_dir().join(relative),
    }
}

pub fn worldgen_dir() -> PathBuf {
    vanilla_dir().join("worldgen")
}

pub fn packs() -> Vec<PathBuf> {
    let Ok(listing) = std::fs::read_dir(assets_dir().join("mcrs/datapacks")) else {
        return Vec::new();
    };
    let mut packs: Vec<PathBuf> = listing
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    packs.sort();
    packs
}

pub fn json_files(dir: &Path) -> Vec<PathBuf> {
    files_with_extension(dir, "json")
}

pub fn nbt_files(dir: &Path) -> Vec<PathBuf> {
    files_with_extension(dir, "nbt")
}

fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, extension, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

pub fn templates() -> BTreeMap<PathBuf, Vec<u8>> {
    let base = vanilla_dir().join("structure");
    let mut templates = BTreeMap::new();
    for path in nbt_files(&base) {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let relative = path
            .strip_prefix(&base)
            .expect("the file is under the base");
        templates.insert(relative.to_owned(), bytes);
    }
    for path in builtin::template_paths("minecraft/structure") {
        let bytes = builtin::asset(&path).expect("a listed built-in template builds");
        let relative = Path::new(&path)
            .strip_prefix("minecraft/structure")
            .expect("a template path is under the structure directory");
        templates.insert(relative.to_owned(), bytes);
    }
    templates
}

pub fn template(id: &ResourceLocationBuf) -> Option<Vec<u8>> {
    let path = format!("{}/structure/{}.nbt", id.namespace(), id.path());
    builtin::asset(&path).or_else(|| std::fs::read(corpus_path(&path)).ok())
}

pub fn is_built_template(id: &ResourceLocationBuf) -> bool {
    builtin::asset(&format!("{}/structure/{}.nbt", id.namespace(), id.path())).is_some()
}

fn id_of(base: &Path, path: &Path) -> ResourceLocationBuf {
    let relative = path.strip_prefix(base).expect("the file is under the base");
    let name = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    ResourceLocationBuf::read(&format!("minecraft:{name}")).expect("a corpus path is a valid id")
}

fn worldgen_roots() -> impl Iterator<Item = PathBuf> {
    std::iter::once(worldgen_dir()).chain(
        packs()
            .into_iter()
            .map(|pack| pack.join("minecraft/worldgen")),
    )
}

pub fn built_template_pools() -> BTreeMap<ResourceLocationBuf, Vec<u8>> {
    built_json("template_pool")
}

pub fn built_biomes() -> BTreeMap<ResourceLocationBuf, Vec<u8>> {
    encoded(builtin::biomes().unwrap_or_else(|error| panic!("{error}")))
}

fn encoded<T: serde::Serialize>(
    entries: BTreeMap<ResourceLocationBuf, T>,
) -> BTreeMap<ResourceLocationBuf, Vec<u8>> {
    entries
        .into_iter()
        .map(|(id, value)| {
            let json =
                serde_json::to_vec(&value).unwrap_or_else(|e| panic!("{id} does not encode: {e}"));
            (id, json)
        })
        .collect()
}

fn built_json(folder: &str) -> BTreeMap<ResourceLocationBuf, Vec<u8>> {
    type Json = LazyLock<BTreeMap<ResourceLocationBuf, Vec<u8>>>;
    static NOISES: Json = LazyLock::new(|| encoded(builtin::noises()));
    static DENSITY_FUNCTIONS: Json = LazyLock::new(|| encoded(builtin::density_functions()));
    static NOISE_SETTINGS: Json = LazyLock::new(|| encoded(builtin::noise_settings()));
    static TEMPLATE_POOLS: Json = LazyLock::new(|| {
        encoded(builtin::template_pools().unwrap_or_else(|error| panic!("{error}")))
    });
    match folder {
        "biome" => built_biomes(),
        "noise" => NOISES.clone(),
        "density_function" => DENSITY_FUNCTIONS.clone(),
        "noise_settings" => NOISE_SETTINGS.clone(),
        "template_pool" => TEMPLATE_POOLS.clone(),
        _ => BTreeMap::new(),
    }
}

fn entries(folder: &str) -> BTreeMap<ResourceLocationBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    let mut shipped: BTreeMap<ResourceLocationBuf, PathBuf> = BTreeMap::new();
    let mut from_packs = Vec::new();
    for (at, root) in worldgen_roots().enumerate() {
        let base = root.join(folder);
        if !base.is_dir() {
            continue;
        }
        for path in json_files(&base) {
            let id = id_of(&base, &path);
            if let Some(first) = shipped.insert(id.clone(), path.clone()) {
                panic!(
                    "{id} is shipped by both {} and {}",
                    first.display(),
                    path.display()
                );
            }
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            if at == 0 {
                entries.insert(id, bytes);
            } else {
                from_packs.push((id, bytes));
            }
        }
    }
    entries.extend(built_json(folder));
    entries.extend(from_packs);
    entries
}

pub fn registry<T: DeserializeOwned>(folder: &str) -> BTreeMap<ResourceLocationBuf, T> {
    entries(folder)
        .into_iter()
        .map(|(id, bytes)| {
            let parsed =
                serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{folder}/{id}: {e}"));
            (id, parsed)
        })
        .collect()
}

pub fn read<T: DeserializeOwned>(folder: &str, id: &ResourceLocationBuf) -> T {
    let path = format!("{}/worldgen/{folder}/{}.json", id.namespace(), id.path());
    let bytes = packs()
        .into_iter()
        .find_map(|root| std::fs::read(root.join(&path)).ok())
        .or_else(|| builtin::asset(&path))
        .or_else(|| built_json(folder).remove(id))
        .or_else(|| std::fs::read(corpus_path(&path)).ok())
        .unwrap_or_else(|| panic!("{path} is neither shipped nor built in"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub fn reencode<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

pub fn same_json(a: &[u8], b: &[u8]) -> bool {
    let parse = |bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok();
    parse(a).is_some_and(|a| Some(a) == parse(b))
}

pub fn round_trips<T: DeserializeOwned + serde::Serialize>(folder: &str) -> usize {
    let entries = entries(folder);
    for (id, bytes) in &entries {
        let raw: serde_json::Value =
            serde_json::from_slice(bytes).unwrap_or_else(|e| panic!("{folder}/{id}: {e}"));
        let parsed: T =
            serde_json::from_slice(bytes).unwrap_or_else(|e| panic!("{folder}/{id}: {e}"));
        assert_eq!(reencode(&parsed), raw, "{folder}/{id} does not round-trip");
    }
    entries.len()
}

pub fn dump_string(r: &mut impl bytes::Buf) -> String {
    let len = r.get_u32_le() as usize;
    String::from_utf8(r.copy_to_bytes(len).to_vec()).unwrap()
}

pub fn dump_placements(r: &mut impl bytes::Buf) -> Vec<([i32; 3], String)> {
    let palette: Vec<String> = (0..r.get_u32_le()).map(|_| dump_string(r)).collect();
    (0..r.get_u32_le())
        .map(|_| {
            let pos = [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
            (pos, palette[r.get_u32_le() as usize].clone())
        })
        .collect()
}
