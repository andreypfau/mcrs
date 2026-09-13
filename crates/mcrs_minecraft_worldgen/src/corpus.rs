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

/// A value as its JSON text reads back. `serde_json::to_value` widens an `f32`
/// to the `f64` nearest its bits; the text form is the shortest decimal that
/// reads back as the same `f32`, which is what the pack shipped.
pub fn reencode<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

/// Every file in one `minecraft/worldgen` folder, parsed and written back,
/// which must equal what was read. Returns how many files were checked, so a
/// caller can pin the count and see a corpus change as a failure.
pub fn round_trips<T: DeserializeOwned + serde::Serialize>(folder: &str) -> usize {
    let base = worldgen_dir().join(folder);
    let paths = json_files(&base);
    for path in &paths {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let raw: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let parsed: T =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            reencode(&parsed),
            raw,
            "{} does not round-trip",
            path.display()
        );
    }
    paths.len()
}

/// A length-prefixed string of one of the oracle's little-endian dumps.
pub fn dump_string(r: &mut impl bytes::Buf) -> String {
    let len = r.get_u32_le() as usize;
    String::from_utf8(r.copy_to_bytes(len).to_vec()).unwrap()
}

/// `SharedConstants.WORLD_VERSION` of the snapshot every oracle dump came from,
/// so a corpus bump cannot silently invalidate a fixture.
pub const WORLD_VERSION: u32 = 5015;

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
    assert_eq!(
        r.get_u32_le(),
        WORLD_VERSION,
        "{} was dumped from a different snapshot than this corpus targets",
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
