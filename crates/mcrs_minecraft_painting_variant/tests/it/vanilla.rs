use std::fs;
use std::path::PathBuf;

use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_painting_variant::PaintingVariant;
use mcrs_minecraft_text::ItemHover;
use serde_json::Value;

#[test]
fn every_vanilla_painting_variant_writes_back_as_its_file() {
    let files: Vec<PathBuf> = fs::read_dir(vanilla_dir().join("painting_variant"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        !files.is_empty(),
        "the data pack ships painting variant files"
    );
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let value: PaintingVariant<ItemHover> =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let file: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_value(&value).unwrap(), file, "{path:?}");
    }
}

#[test]
fn a_side_outside_one_to_sixteen_is_refused() {
    for side in [0, 17] {
        let json = format!(r#"{{"asset_id":"minecraft:kebab","height":1,"width":{side}}}"#);
        let read = serde_json::from_str::<PaintingVariant<ItemHover>>(&json);
        assert!(read.is_err(), "width {side} is outside 1 to 16");
    }
}
