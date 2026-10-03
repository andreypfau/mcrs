use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mcrs_minecraft_block::definition::schema::{BlockDefinitionFile, Description};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::corpus;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportBlock {
    #[allow(dead_code)]
    properties: Option<IgnoredAny>,
    states: Vec<ReportState>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportState {
    id: u32,
    #[serde(default)]
    default: bool,
    #[serde(default)]
    properties: BTreeMap<String, String>,
}

type Report = BTreeMap<String, ReportBlock>;

fn parse<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| corpus::io(path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn descriptions(directory: &Path) -> Result<Vec<Description>, String> {
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| corpus::io(directory, error))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| corpus::io(directory, error))?;
    paths.retain(|path| path.extension().is_some_and(|e| e == "json"));
    paths.sort();
    paths
        .iter()
        .map(|path| parse::<BlockDefinitionFile>(path).map(|file| file.block.description))
        .collect()
}

// The report sorts the keys of a block's `properties`, so the declared order
// is only recoverable through each state's own property map.
fn state_index(description: &Description, state: &ReportState) -> Result<u32, String> {
    let properties = &description.properties.0;
    if state.properties.len() != properties.len() {
        return Err(format!(
            "state {} names {} properties, the definition declares {}",
            state.id,
            state.properties.len(),
            properties.len()
        ));
    }
    let mut index = 0u32;
    for property in properties {
        let text = state
            .properties
            .get(&*property.name)
            .ok_or_else(|| format!("state {} has no value for `{}`", state.id, property.name))?;
        let position = property
            .values
            .iter()
            .position(|value| value.renders_to(text))
            .ok_or_else(|| {
                format!(
                    "state {}: `{}` has no value `{text}` in the definition",
                    state.id, property.name
                )
            })?;
        index = index * property.values.len() as u32 + position as u32;
    }
    Ok(index)
}

fn mismatches(report: &Report, descriptions: &[Description]) -> Vec<String> {
    let mut mismatches = Vec::new();

    let mut claimed: BTreeMap<u32, &str> = BTreeMap::new();
    for (name, block) in report {
        for state in &block.states {
            if let Some(other) = claimed.insert(state.id, name) {
                mismatches.push(format!(
                    "state {} is claimed by {other} and {name}",
                    state.id
                ));
            }
        }
    }
    if claimed.is_empty() {
        mismatches.push("the report lists no state".to_owned());
    }
    for expected in 0..claimed.len() as u32 {
        if !claimed.contains_key(&expected) {
            mismatches.push(format!("state {expected} is claimed by no block"));
        }
    }

    let mut defined: BTreeSet<&str> = BTreeSet::new();
    for description in descriptions {
        let name = description.identifier.as_str();
        if !defined.insert(name) {
            mismatches.push(format!("{name}: defined twice"));
        }
    }
    let reported: BTreeSet<&str> = report.keys().map(String::as_str).collect();
    mismatches.extend(
        defined
            .difference(&reported)
            .map(|name| format!("{name}: defined, not in the report")),
    );
    mismatches.extend(
        reported
            .difference(&defined)
            .map(|name| format!("{name}: in the report, not defined")),
    );

    for description in descriptions {
        let name = &description.identifier;
        let Some(block) = report.get(name.as_str()) else {
            continue;
        };
        let expected = description.properties.state_count();
        if block.states.len() != expected {
            mismatches.push(format!(
                "{name}: {} reported states, definition {expected}",
                block.states.len()
            ));
        }
        for state in &block.states {
            match state_index(description, state) {
                Ok(index) => {
                    let expected = description.base_state_id as u32 + index;
                    if state.id != expected {
                        mismatches.push(format!("{name}: state {} should be {expected}", state.id));
                    }
                }
                Err(error) => mismatches.push(format!("{name}: {error}")),
            }
        }
        let defaults: Vec<u32> = block
            .states
            .iter()
            .filter(|state| state.default)
            .map(|state| state.id)
            .collect();
        if defaults != [description.default_state_id as u32] {
            mismatches.push(format!(
                "{name}: reported defaults {defaults:?}, definition {}",
                description.default_state_id
            ));
        }
    }
    mismatches
}

pub fn check(report: &Path, definitions: &Path) -> Result<(), String> {
    let mismatches = mismatches(&parse(report)?, &descriptions(definitions)?);
    if mismatches.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} disagrees with {} in {} places:\n{}",
        definitions.display(),
        report.display(),
        mismatches.len(),
        mismatches.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testing::scratch;

    const REPORT: &str = r#"{
        "minecraft:air": {"states": [{"id": 0, "default": true}]},
        "minecraft:chest": {
            "properties": {"type": ["single", "left"], "lit": ["true", "false"]},
            "states": [
                {"id": 1, "properties": {"lit": "true", "type": "single"}},
                {"id": 2, "properties": {"lit": "true", "type": "left"}},
                {"id": 3, "properties": {"lit": "false", "type": "single"}, "default": true},
                {"id": 4, "properties": {"lit": "false", "type": "left"}}
            ]
        }
    }"#;
    const AIR: &str = r#""identifier": "minecraft:air", "protocol_id": 0,
        "base_state_id": 0, "default_state_id": 0"#;
    const CHEST: &str = r#""identifier": "minecraft:chest", "protocol_id": 1,
        "properties": {"lit": [true, false], "type": ["single", "left"]},
        "base_state_id": 1, "default_state_id": 3"#;

    fn definition(description: &str) -> String {
        format!(
            r#"{{"format_version": "1", "minecraft:block": {{"description": {{{description}}}}}}}"#
        )
    }

    fn found(report: &str, descriptions: &[&str]) -> Vec<String> {
        let descriptions: Vec<Description> = descriptions
            .iter()
            .map(|description| {
                serde_json::from_str::<BlockDefinitionFile>(&definition(description))
                    .unwrap()
                    .block
                    .description
            })
            .collect();
        mismatches(&serde_json::from_str(report).unwrap(), &descriptions)
    }

    #[test]
    fn definitions_that_agree_with_the_report_yield_nothing() {
        assert_eq!(found(REPORT, &[AIR, CHEST]), Vec::<String>::new());
    }

    #[test]
    fn a_reversed_property_order_moves_the_state_ids() {
        let reversed = CHEST.replace(
            r#""lit": [true, false], "type": ["single", "left"]"#,
            r#""type": ["single", "left"], "lit": [true, false]"#,
        );
        assert_eq!(
            found(REPORT, &[AIR, &reversed]),
            [
                "minecraft:chest: state 2 should be 3",
                "minecraft:chest: state 3 should be 2",
            ]
        );
    }

    #[test]
    fn a_different_default_state_is_reported() {
        let moved = CHEST.replace(r#""default_state_id": 3"#, r#""default_state_id": 1"#);
        assert_eq!(
            found(REPORT, &[AIR, &moved]),
            ["minecraft:chest: reported defaults [3], definition 1"]
        );
    }

    #[test]
    fn a_property_value_the_report_lacks_changes_the_state_count() {
        let wider = CHEST.replace(r#"["single", "left"]"#, r#"["single", "left", "right"]"#);
        let found = found(REPORT, &[AIR, &wider]);
        assert_eq!(
            found[0],
            "minecraft:chest: 4 reported states, definition 6"
        );
    }

    #[test]
    fn a_property_declared_with_another_type_has_no_such_value() {
        let stringly = CHEST.replace("[true, false]", r#"["yes", "no"]"#);
        let found = found(REPORT, &[AIR, &stringly]);
        assert!(
            found.contains(
                &"minecraft:chest: state 1: `lit` has no value `true` in the definition".to_owned()
            ),
            "{found:?}"
        );
    }

    #[test]
    fn a_block_on_one_side_only_is_reported_from_both_directions() {
        let stone = r#""identifier": "minecraft:stone", "protocol_id": 2,
            "base_state_id": 5, "default_state_id": 5"#;
        assert_eq!(
            found(REPORT, &[AIR, stone, stone]),
            [
                "minecraft:stone: defined twice",
                "minecraft:stone: defined, not in the report",
                "minecraft:chest: in the report, not defined",
            ]
        );
    }

    #[test]
    fn a_gap_an_overlap_and_an_empty_report_are_reported() {
        let report = r#"{
            "minecraft:air": {"states": [{"id": 0, "default": true}]},
            "minecraft:cave_air": {"states": [{"id": 0, "default": true}]},
            "minecraft:stone": {"states": [{"id": 3, "default": true}]}
        }"#;
        assert_eq!(found("{}", &[]), ["the report lists no state"]);
        assert_eq!(
            found(report, &[])[..2],
            [
                "state 0 is claimed by minecraft:air and minecraft:cave_air",
                "state 1 is claimed by no block",
            ]
        );
    }

    #[test]
    fn check_reads_the_report_and_every_definition_file_and_names_both_on_failure() {
        let dir = scratch("blocks-check");
        let definitions = dir.join("block_definition");
        fs::create_dir_all(&definitions).unwrap();
        let report = dir.join("blocks.json");
        fs::write(&report, REPORT).unwrap();
        fs::write(definitions.join("README.md"), "not a definition").unwrap();
        fs::write(definitions.join("air.json"), definition(AIR)).unwrap();
        fs::write(definitions.join("chest.json"), definition(CHEST)).unwrap();
        assert_eq!(check(&report, &definitions), Ok(()));

        fs::remove_file(definitions.join("chest.json")).unwrap();
        let error = check(&report, &definitions).unwrap_err();
        assert!(error.contains("blocks.json"), "{error}");
        assert!(error.contains("block_definition"), "{error}");
        assert!(
            error.ends_with("minecraft:chest: in the report, not defined"),
            "{error}"
        );
    }
}
