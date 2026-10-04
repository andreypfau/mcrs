//! Reading the shipped asset corpus off disk, for the tests that check the
//! engine against every file the game ships rather than against a fixture.

use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_registry::{Registry, RegistrySet};
use mcrs_minecraft_worldgen_builtin as builtin;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub fn worldgen_dir() -> PathBuf {
    assets_dir().join("minecraft/worldgen")
}

/// Every data pack layered over the vanilla tree, in name order.
fn packs() -> Vec<PathBuf> {
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

/// Every `.json` under `dir`, recursively, in a stable order.
pub fn json_files(dir: &Path) -> Vec<PathBuf> {
    files_with_extension(dir, "json")
}

/// Every `.nbt` under `dir`, recursively, in a stable order.
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

/// Every structure template as the gzip NBT it ships as, keyed by its path
/// under `minecraft/structure`: the files the corpus ships, and the built-in
/// ones it ships no file for.
pub fn templates() -> BTreeMap<PathBuf, Vec<u8>> {
    let base = assets_dir().join("minecraft/structure");
    let mut templates: BTreeMap<PathBuf, Vec<u8>> = builtin::paths("minecraft/structure")
        .into_iter()
        .map(|path| {
            let bytes = builtin::asset(&path).expect("a listed built-in template builds");
            let relative = Path::new(&path)
                .strip_prefix("minecraft/structure")
                .expect("a template path is under the structure directory");
            (relative.to_owned(), bytes)
        })
        .collect();
    for path in nbt_files(&base) {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let relative = path
            .strip_prefix(&base)
            .expect("the file is under the base");
        templates.insert(relative.to_owned(), bytes);
    }
    templates
}

/// One structure template as the gzip NBT it ships as: the corpus file, or the
/// built-in when the corpus ships none.
pub fn template(id: &ResourceLocation) -> Option<Vec<u8>> {
    let shipped = assets_dir()
        .join(id.namespace())
        .join("structure")
        .join(format!("{}.nbt", id.path()));
    std::fs::read(shipped)
        .ok()
        .or_else(|| builtin::asset(&format!("{}/structure/{}.nbt", id.namespace(), id.path())))
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

/// One `minecraft/worldgen` registry as the JSON each entry ships as: the
/// built-in entries, overridden by the files of the vanilla tree and of every
/// pack. An id two files ship is refused.
fn entries(folder: &str) -> BTreeMap<ResourceLocation, Vec<u8>> {
    let mut entries = builtin::assets(folder);
    let mut shipped: BTreeMap<ResourceLocation, PathBuf> = BTreeMap::new();
    let roots = std::iter::once(worldgen_dir()).chain(
        packs()
            .into_iter()
            .map(|pack| pack.join("minecraft/worldgen")),
    );
    for root in roots {
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
            entries.insert(id, bytes);
        }
    }
    entries
}

/// A registry set holding the one registry `R`, numbered by the files the
/// corpus ships under `assets/minecraft/<folder>`, for reading a value that
/// names an entry of it inside `RegistrySet::scope`.
pub fn shipped_registry_set<R: RegistryKey>(folder: &str) -> RegistrySet {
    let root = assets_dir().join("minecraft").join(folder);
    let names = json_files(&root).into_iter().map(|file| {
        let relative = file
            .strip_prefix(&root)
            .expect("a corpus file is under its folder")
            .with_extension("");
        ResourceLocation::minecraft(&relative.to_string_lossy().replace('\\', "/"))
    });
    let registry = Registry::<R>::new(names, std::iter::empty())
        .unwrap_or_else(|e| panic!("{folder} does not number: {e}"));
    RegistrySet::new()
        .with(registry)
        .unwrap_or_else(|e| panic!("{folder} does not join the set: {e}"))
}

/// One `minecraft/worldgen` registry, parsed. Every entry must parse: dropping
/// the ones that do not would let a test read "the whole corpus compiles" off a
/// corpus quietly missing the entries that broke.
pub fn registry<T: DeserializeOwned>(folder: &str) -> BTreeMap<ResourceLocation, T> {
    entries(folder)
        .into_iter()
        .map(|(id, bytes)| {
            let parsed =
                serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{folder}/{id}: {e}"));
            (id, parsed)
        })
        .collect()
}

/// Every `.json` under `assets/<dir>`, parsed. Panics naming every file that
/// did not parse, or if the folder holds none.
pub fn parse_all<T: DeserializeOwned>(dir: &str) -> Vec<(PathBuf, T)> {
    let mut parsed = Vec::new();
    let mut failures = Vec::new();
    for path in json_files(&assets_dir().join(dir)) {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        match serde_json::from_slice::<T>(&bytes) {
            Ok(value) => parsed.push((path, value)),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} files in {dir} failed to parse:\n{}",
        failures.len(),
        failures.len() + parsed.len(),
        failures.join("\n")
    );
    assert!(!parsed.is_empty(), "no files found in {dir}");
    parsed
}

/// One named `minecraft/worldgen` asset, which must parse: the file the vanilla
/// tree or a pack ships, or the built-in entry where none ships it.
pub fn read<T: DeserializeOwned>(folder: &str, id: &ResourceLocation) -> T {
    let path = format!("{}/worldgen/{folder}/{}.json", id.namespace(), id.path());
    let bytes = std::iter::once(assets_dir())
        .chain(packs())
        .find_map(|root| std::fs::read(root.join(&path)).ok())
        .or_else(|| builtin::asset(&path))
        .unwrap_or_else(|| panic!("{path} is neither shipped nor built in"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A value as its JSON text reads back. `serde_json::to_value` widens an `f32`
/// to the `f64` nearest its bits; the text form is the shortest decimal that
/// reads back as the same `f32`, which is what the pack shipped.
pub fn reencode<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

/// Every entry of one `minecraft/worldgen` registry, shipped or built in,
/// parsed and written back, which must equal what was read. Returns how many
/// entries were checked, so a caller can pin the count and see a corpus change
/// as a failure.
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

/// A length-prefixed string of one of the oracle's little-endian dumps.
pub fn dump_string(r: &mut impl bytes::Buf) -> String {
    let len = r.get_u32_le() as usize;
    String::from_utf8(r.copy_to_bytes(len).to_vec()).unwrap()
}

/// One of the oracle's little-endian dumps past its header: the eight-byte
/// `magic`, format version 1, and the world version.
pub fn open_dump(path: &Path, magic: &[u8; 8]) -> bytes::Bytes {
    use bytes::Buf;
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut r = bytes::Bytes::from(data);
    assert_eq!(
        r.copy_to_bytes(8).as_ref(),
        magic,
        "{} is not a {} dump",
        path.display(),
        String::from_utf8_lossy(magic)
    );
    assert_eq!(r.get_u32_le(), 1, "unsupported oracle format version");
    let found = r.get_u32_le();
    let expected = u32::try_from(VERSION.world_version).expect("the world version is negative");
    assert_eq!(
        found,
        expected,
        "{}: dumped at world version {found}, the corpus is at {expected}",
        path.display()
    );
    r
}

/// A palette of state names followed by positions indexing into it, as the
/// oracle dumps every list of placed blocks.
pub fn dump_placements(r: &mut impl bytes::Buf) -> Vec<([i32; 3], String)> {
    let palette: Vec<String> = (0..r.get_u32_le()).map(|_| dump_string(r)).collect();
    (0..r.get_u32_le())
        .map(|_| {
            let pos = [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
            (pos, palette[r.get_u32_le() as usize].clone())
        })
        .collect()
}
