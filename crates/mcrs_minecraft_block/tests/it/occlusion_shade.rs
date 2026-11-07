use std::collections::HashMap;

use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags};
use mcrs_voxel::BlockStateId;
use serde::Deserialize;

use crate::common::{assert_no_mismatches, corpus};

const ARTEFACT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../mcrs_minecraft_mesh/tests/fixtures/vanilla/block_occlusion_shade.json"
));

#[derive(Deserialize)]
struct Artefact {
    exceptions: Vec<Exception>,
}

#[derive(Deserialize)]
struct Exception {
    state: String,
    full_collision: bool,
    occlusion_shade: f32,
}

fn state_name(definitions: &BlockDefinitions, state: BlockStateId) -> String {
    let Some(entry) = definitions.owner(state) else {
        return String::new();
    };
    let properties: Vec<String> = entry
        .properties
        .0
        .iter()
        .filter_map(|property| {
            let value = entry.value_of(state, &property.name)?;
            Some(format!("{}={}", property.name, value.to_text()))
        })
        .collect();
    if properties.is_empty() {
        entry.identifier.to_string()
    } else {
        format!("{}[{}]", entry.identifier, properties.join(","))
    }
}

#[test]
fn each_state_has_the_occlusion_shade_of_the_game() {
    let artefact: Artefact = serde_json::from_str(ARTEFACT).expect("the shade artefact parses");
    let mut exceptions: HashMap<&str, &Exception> = artefact
        .exceptions
        .iter()
        .map(|exception| (exception.state.as_str(), exception))
        .collect();
    let definitions = corpus();
    let mut mismatches = Vec::new();
    for state in definitions.states() {
        let name = state_name(definitions, state);
        let data = definitions.state(state);
        let full_collision = data
            .flags
            .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK);
        let expected = match exceptions.remove(name.as_str()) {
            Some(exception) => {
                if exception.full_collision != full_collision {
                    mismatches.push(format!("{name}: full collision {full_collision}"));
                }
                exception.occlusion_shade
            }
            None if full_collision => 0.2,
            None => 1.0,
        };
        if data.occlusion_shade() != expected {
            mismatches.push(format!(
                "{name}: {} where the game has {expected}",
                data.occlusion_shade()
            ));
        }
    }
    mismatches.extend(
        exceptions
            .keys()
            .map(|name| format!("{name}: no such state in the corpus")),
    );
    assert_no_mismatches("occlusion shades", mismatches);
}
