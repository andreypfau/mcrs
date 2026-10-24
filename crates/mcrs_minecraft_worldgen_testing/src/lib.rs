//! Reading the game's data pack and the repository's assets off disk, for the
//! tests that check the engine against every file the game ships rather than
//! against a fixture.
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

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_builtin as builtin;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub use mcrs_minecraft_fixtures::vanilla_dir;

/// The repository's assets: the reports, the definitions and the local packs.
pub fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// `relative` below a pack root, with the vanilla pack's `minecraft` directory read from the
/// game's data pack and everything else from [`assets_dir`].
pub fn corpus_path(relative: &str) -> PathBuf {
    match Path::new(relative).strip_prefix("minecraft").ok() {
        Some(below) => vanilla_dir().join(below),
        None => assets_dir().join(relative),
    }
}

pub fn worldgen_dir() -> PathBuf {
    vanilla_dir().join("worldgen")
}

/// Every data pack layered over the vanilla tree, in name order.
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

/// Every structure template as the gzip NBT the loader reads, keyed by its path
/// under `minecraft/structure`: the built-in ones, and the game's files the code
/// builds none for.
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

/// One structure template as the gzip NBT the loader reads: the built-in, or the
/// game's file when the code builds none.
pub fn template(id: &ResourceLocation) -> Option<Vec<u8>> {
    let path = format!("{}/structure/{}.nbt", id.namespace(), id.path());
    builtin::asset(&path).or_else(|| std::fs::read(corpus_path(&path)).ok())
}

/// Whether the code builds the structure template `id` in place of the game's file.
pub fn is_built_template(id: &ResourceLocation) -> bool {
    builtin::asset(&format!("{}/structure/{}.nbt", id.namespace(), id.path())).is_some()
}

/// The id a corpus file carries: its path under `base`, without the extension.
fn id_of(base: &Path, path: &Path) -> ResourceLocation {
    let relative = path.strip_prefix(base).expect("the file is under the base");
    let name = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    ResourceLocation::read(&format!("minecraft:{name}")).expect("a corpus path is a valid id")
}

fn worldgen_roots() -> impl Iterator<Item = PathBuf> {
    std::iter::once(worldgen_dir()).chain(
        packs()
            .into_iter()
            .map(|pack| pack.join("minecraft/worldgen")),
    )
}

/// The built template pools as the JSON they encode to, for the tests that read
/// the corpus as files.
pub fn built_template_pools() -> BTreeMap<ResourceLocation, Vec<u8>> {
    built_json("template_pool")
}

/// The built biomes as the JSON they encode to, for the tests that read the
/// corpus as files.
pub fn built_biomes() -> BTreeMap<ResourceLocation, Vec<u8>> {
    encoded(builtin::biomes().unwrap_or_else(|error| panic!("{error}")))
}

fn encoded<T: serde::Serialize>(
    entries: BTreeMap<ResourceLocation, T>,
) -> BTreeMap<ResourceLocation, Vec<u8>> {
    entries
        .into_iter()
        .map(|(id, value)| {
            let json =
                serde_json::to_vec(&value).unwrap_or_else(|e| panic!("{id} does not encode: {e}"));
            (id, json)
        })
        .collect()
}

/// The built-in entries of one `minecraft/worldgen` folder as the JSON they
/// encode to, for the tests that read the corpus as files.
fn built_json(folder: &str) -> BTreeMap<ResourceLocation, Vec<u8>> {
    type Json = LazyLock<BTreeMap<ResourceLocation, Vec<u8>>>;
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

/// One `minecraft/worldgen` registry as the JSON the loader reads for each
/// entry: the game's files, replaced by the built-in entries, replaced by the
/// files of every pack. An id two packs ship, or a pack and the game, is refused.
fn entries(folder: &str) -> BTreeMap<ResourceLocation, Vec<u8>> {
    let mut entries = BTreeMap::new();
    let mut shipped: BTreeMap<ResourceLocation, PathBuf> = BTreeMap::new();
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

/// Every `.json` under the [`corpus_path`] of `dir`, parsed. Panics naming every
/// file that did not parse, or if the folder holds none.
pub fn parse_all<T: DeserializeOwned>(dir: &str) -> Vec<(PathBuf, T)> {
    let mut parsed = Vec::new();
    let mut failures = Vec::new();
    for path in json_files(&corpus_path(dir)) {
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

/// One named `minecraft/worldgen` asset, which must parse: the file a pack
/// ships, else the built-in entry, else the game's file.
pub fn read<T: DeserializeOwned>(folder: &str, id: &ResourceLocation) -> T {
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

/// A value as its JSON text reads back. `serde_json::to_value` widens an `f32`
/// to the `f64` nearest its bits; the text form is the shortest decimal that
/// reads back as the same `f32`, which is what the pack shipped.
pub fn reencode<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

/// Whether two JSON texts hold the same value, whatever their layout.
pub fn same_json(a: &[u8], b: &[u8]) -> bool {
    let parse = |bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok();
    parse(a).is_some_and(|a| Some(a) == parse(b))
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
