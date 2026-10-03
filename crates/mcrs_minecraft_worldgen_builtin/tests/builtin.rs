use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fmt::Debug;

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
}

#[test]
fn density_functions_read_back() {
    reads_back::<DensityFunctionHolder>("density_function", builtin::density_functions(), 65, &[]);
}

#[test]
fn noise_settings_read_back() {
    reads_back::<NoiseGeneratorSettings>("noise_settings", builtin::noise_settings(), 8, &[]);
}

#[test]
fn noises_read_back() {
    reads_back::<NoiseParam>("noise", builtin::noises(), 69, MISREAD_NOISES);
}

#[test]
fn a_path_outside_the_built_in_folders_is_not_served() {
    assert_eq!(builtin::asset("minecraft/worldgen/biome/plains.json"), None);
    assert_eq!(
        builtin::asset("minecraft/worldgen/noise/no_such_noise.json"),
        None
    );
}
