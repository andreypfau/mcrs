use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Cursor;

use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_worldgen_feature::template::Template;

// chisle: `serde_json` without `float_roundtrip` reads these base amplitudes one
// ulp low, from a file and from a built-in alike. The feature cannot simply be
// enabled: it turns an overflowing `f32` into an error where item components
// expect infinity.
const MISREAD_NOISES: &[&str] = &[
    "jagged",
    "nether/temperature",
    "nether/vegetation",
    "offset",
    "pillar",
    "ridge",
];

/// A built-in reaches the loaders as the JSON it encodes to, so the encoding
/// has to read back as the value that was built.
fn reads_back<T: DeserializeOwned + PartialEq + Debug>(
    folder: &str,
    built: BTreeMap<ResourceLocation, T>,
    expected: usize,
    misread: &[&str],
) {
    assert_eq!(built.len(), expected, "{folder}");
    let mut encoded = builtin::assets(folder);
    for (id, value) in built {
        let bytes = encoded
            .remove(&id)
            .unwrap_or_else(|| panic!("{folder}/{id} is not served"));
        let read: T =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{folder}/{id}: {e}"));
        assert_eq!(
            read == value,
            !misread.contains(&id.path()),
            "{folder}/{id}"
        );
        let path = format!("{}/worldgen/{folder}/{}.json", id.namespace(), id.path());
        assert_eq!(builtin::asset(&path), Some(bytes), "{path}");
    }
    assert!(
        encoded.is_empty(),
        "{folder} serves entries that were not built"
    );
    let listed = builtin::paths(&format!("minecraft/worldgen/{folder}"));
    assert!(listed.len() <= expected && !listed.is_empty(), "{folder}");
    for path in listed {
        assert!(
            builtin::asset(&path).is_some(),
            "{path} is listed and not served"
        );
    }
}

#[test]
fn noises_read_back() {
    reads_back::<NoiseParam>("noise", builtin::noises(), 69, MISREAD_NOISES);
}

#[test]
fn a_path_outside_the_built_in_folders_is_not_served() {
    assert_eq!(
        builtin::asset("minecraft/worldgen/structure/igloo.json"),
        None
    );
    assert_eq!(
        builtin::asset("minecraft/worldgen/noise/no_such_noise.json"),
        None
    );
}

#[test]
fn every_template_reads_back_from_the_bytes_it_is_served_as() {
    let built = builtin::templates();
    assert_eq!(built.len(), 483);
    let listed = builtin::paths("minecraft/structure");
    assert_eq!(listed.len(), built.len());
    for (id, template) in built {
        let path = format!("minecraft/structure/{}.nbt", id.path());
        assert!(listed.contains(&path), "{path} is not listed");
        let bytes = builtin::asset(&path).unwrap_or_else(|| panic!("{path} is not served"));
        let read: Template = from_gzip_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(read, template, "{path}");
    }
    assert!(builtin::asset("minecraft/structure/igloo/top.nbt").is_none());
}

fn digest(entries: impl IntoIterator<Item = (ResourceLocation, Vec<u8>)>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (id, bytes) in entries {
        hasher.update(id.as_str());
        hasher.update([0]);
        hasher.update(&bytes);
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Reading back only proves an entry survives its own encoding. These pin the
/// encoding itself, which matched the game's files entry for entry when the
/// files were removed, so a change to a description shows here.
#[test]
fn the_biomes_template_pools_and_templates_are_the_ones_that_matched_the_game() {
    assert_eq!(
        digest(builtin::assets("biome")),
        "f4812ebae49ea10e97ff9b40caa24de7507e0db5285501fb3cc4420c8530ae0f"
    );
    assert_eq!(
        digest(builtin::assets("template_pool")),
        "f13af32a6f2766c69986cd799dcf0ee948023bf3592ca34c1a15d062a7620f2d"
    );
    let templates = builtin::templates().into_iter().map(|(id, template)| {
        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes(&template, &mut nbt).unwrap();
        (id, nbt)
    });
    assert_eq!(
        digest(templates),
        "2cca7eff3d5c0fd69e04a71961f2459c37f0f37a56bd94c9f7ceef4bac222410"
    );
}
