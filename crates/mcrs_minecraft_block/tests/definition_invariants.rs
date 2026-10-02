use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mcrs_minecraft_block::definition::CORPUS_DIRECTORY;
use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[allow(dead_code)]
    format_version: IgnoredAny,
    #[serde(rename = "minecraft:block")]
    block: Block,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    description: Description,
    #[serde(default)]
    components: BTreeMap<String, IgnoredAny>,
    #[serde(default)]
    permutations: Vec<Permutation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    identifier: String,
    #[allow(dead_code)]
    properties: Option<IgnoredAny>,
    #[allow(dead_code)]
    protocol_id: IgnoredAny,
    #[allow(dead_code)]
    base_state_id: IgnoredAny,
    #[allow(dead_code)]
    default_state_id: IgnoredAny,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Permutation {
    #[allow(dead_code)]
    condition: IgnoredAny,
    components: BTreeMap<String, IgnoredAny>,
}

fn files() -> Vec<(PathBuf, File)> {
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

#[test]
fn a_definition_file_is_named_after_its_identifier() {
    let files = files();
    assert!(!files.is_empty(), "no block definition was read");
    let mismatches: Vec<String> = files
        .iter()
        .filter_map(|(path, file)| {
            let identifier = &file.block.description.identifier;
            let expected = format!("minecraft:{}", path.file_stem()?.to_str()?);
            (*identifier != expected)
                .then(|| format!("{}: identifier is {identifier}", path.display()))
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} files not named after their identifier:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn a_component_is_not_stated_both_for_the_block_and_in_a_permutation() {
    let files = files();
    assert!(
        files
            .iter()
            .any(|(_, file)| !file.block.permutations.is_empty()),
        "no block with a permutation was read"
    );
    let mut mismatches = Vec::new();
    for (path, file) in &files {
        for permutation in &file.block.permutations {
            for key in permutation.components.keys() {
                if file.block.components.contains_key(key) {
                    mismatches.push(format!(
                        "{}: {key} is in the block and in a permutation",
                        path.display()
                    ));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} components stated twice:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
