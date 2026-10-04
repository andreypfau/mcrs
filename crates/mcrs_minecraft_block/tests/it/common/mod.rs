use std::path::{Path, PathBuf};

use mcrs_minecraft_block::definition::CORPUS_DIRECTORY;
use serde::de::DeserializeOwned;

pub fn definition_files<T: DeserializeOwned>() -> Vec<(PathBuf, T)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(CORPUS_DIRECTORY);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|e| panic!("{}: {e}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let file = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            (path, file)
        })
        .collect()
}

pub fn assert_no_mismatches(what: &str, mismatches: Vec<String>) {
    assert!(
        mismatches.is_empty(),
        "{} {what}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
