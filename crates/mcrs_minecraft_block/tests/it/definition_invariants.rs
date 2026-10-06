use mcrs_minecraft_registry::DenseId;
use std::collections::BTreeMap;
use std::path::PathBuf;

use mcrs_minecraft_block::keys::Block as BlockKey;
use mcrs_minecraft_registry::Id;
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
    crate::common::definition_files()
}

#[test]
fn a_definition_file_is_named_after_its_identifier() {
    let files = files();
    assert!(!files.is_empty(), "no block definition was read");
    let mismatches = files
        .iter()
        .filter_map(|(path, file)| {
            let identifier = &file.block.description.identifier;
            let expected = format!("minecraft:{}", path.file_stem()?.to_str()?);
            (*identifier != expected)
                .then(|| format!("{}: identifier is {identifier}", path.display()))
        })
        .collect();
    crate::common::assert_no_mismatches("files not named after their identifier", mismatches);
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
    crate::common::assert_no_mismatches("components stated twice", mismatches);
}

#[test]
fn default_state_by_id_equals_by_name_for_every_block() {
    let blocks = crate::common::corpus();
    let mismatches: Vec<String> = (0..blocks.blocks().len())
        .map(|number| {
            mcrs_minecraft_block::keys::Block::from_id(Id::<BlockKey>::from_raw(number as u16))
                .expect("every block of the corpus has a generated key")
        })
        .filter(|key| {
            blocks.default_state_of(key.id()) != blocks.default_state(key.as_static_str())
        })
        .map(|key| key.as_static_str().to_owned())
        .collect();
    crate::common::assert_no_mismatches(
        "blocks whose id and name reach different default states",
        mismatches,
    );
}
