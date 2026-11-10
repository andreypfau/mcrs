use std::fs;
use std::path::PathBuf;

use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_text::ItemHover;
use mcrs_minecraft_trim::{TrimMaterial, TrimPattern};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[test]
fn every_vanilla_trim_file_writes_back_as_read() {
    fn check<T: Serialize + DeserializeOwned>(registry: &str) {
        let files: Vec<PathBuf> = fs::read_dir(vanilla_dir().join(registry))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(!files.is_empty(), "the data pack ships {registry} files");
        for path in files {
            let bytes = fs::read(&path).unwrap();
            let value: T =
                serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path:?}: {error}"));
            let file: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(serde_json::to_value(&value).unwrap(), file, "{path:?}");
        }
    }

    check::<TrimMaterial<ItemHover>>("trim_material");
    check::<TrimPattern<ItemHover>>("trim_pattern");
}

#[test]
fn a_trim_pattern_without_decal_reads_as_no_decal_and_writes_it() {
    let json = r#"{"asset_id":"minecraft:coast","description":"Coast"}"#;
    let pattern: TrimPattern<ItemHover> = serde_json::from_str(json).unwrap();
    assert!(!pattern.decal, "an absent decal reads as false");
    assert_eq!(
        serde_json::to_string(&pattern).unwrap(),
        r#"{"asset_id":"minecraft:coast","decal":false,"description":"Coast"}"#
    );
}
