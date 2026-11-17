#![expect(
    clippy::disallowed_types,
    reason = "the written form is compared with the file as JSON"
)]

use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::vanilla_dir;

pub fn assert_vanilla_round_trip<T: Serialize + DeserializeOwned>(registry: &str) {
    let files: Vec<PathBuf> = fs::read_dir(vanilla_dir().join(registry))
        .unwrap_or_else(|error| panic!("{registry}: {error}"))
        .map(|entry| entry.expect("the registry directory lists").path())
        .collect();
    assert!(!files.is_empty(), "the data pack ships {registry} files");
    for path in files {
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let value: T =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let file: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let written: Value = serde_json::to_value(&value).expect("a value writes as JSON");
        assert_eq!(written, file, "{path:?}");
    }
}
