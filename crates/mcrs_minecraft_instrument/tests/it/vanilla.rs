use std::fs;
use std::path::PathBuf;

use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_instrument::Instrument;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::ItemHover;
use serde_json::Value;

#[test]
fn every_vanilla_instrument_writes_back_as_its_file() {
    let files: Vec<PathBuf> = fs::read_dir(vanilla_dir().join("instrument"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(!files.is_empty(), "the data pack ships instrument files");
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let value: Instrument<ItemHover, Named> =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let file: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_value(&value).unwrap(), file, "{path:?}");
    }
}
