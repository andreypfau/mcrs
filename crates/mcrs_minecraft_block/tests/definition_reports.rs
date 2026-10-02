use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use mcrs_minecraft_block::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_block::definition::schema::{BlockDefinitionFile, Description};
use mcrs_minecraft_registry::StaticRegistryTable;
use serde::Deserialize;
use serde::de::IgnoredAny;

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

struct Corpus {
    definitions: Vec<(PathBuf, BlockDefinitionFile)>,
    report: BTreeMap<String, ReportBlock>,
    registries: StaticRegistryTable,
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn read(path: &PathBuf) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let directory = assets().join(CORPUS_DIRECTORY);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
            .unwrap_or_else(|e| panic!("{}: {e}", directory.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort();
        let definitions = paths
            .into_iter()
            .map(|path| {
                let file = serde_json::from_slice(&read(&path))
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                (path, file)
            })
            .collect();

        let report_path = assets().join("mcrs/reports/blocks.json");
        let report = serde_json::from_slice(&read(&report_path))
            .unwrap_or_else(|e| panic!("{}: {e}", report_path.display()));

        let registries_path = assets().join("mcrs/reports/registries.json");
        let registries = StaticRegistryTable::load(&registries_path)
            .unwrap_or_else(|e| panic!("{}: {e}", registries_path.display()));

        Corpus {
            definitions,
            report,
            registries,
        }
    })
}

fn descriptions() -> impl Iterator<Item = &'static Description> {
    corpus()
        .definitions
        .iter()
        .map(|(_, file)| &file.block.description)
}

fn assert_no_mismatches(what: &str, mismatches: Vec<String>) {
    assert!(
        mismatches.is_empty(),
        "{} {what}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

fn reported<'a>(
    description: &Description,
    report: &'a BTreeMap<String, ReportBlock>,
) -> Option<&'a ReportBlock> {
    report.get(description.identifier.as_str())
}

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

#[test]
fn state_ids_follow_the_declared_property_order() {
    let corpus = corpus();
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for description in descriptions() {
        let Some(block) = reported(description, &corpus.report) else {
            continue;
        };
        seen += 1;
        for state in &block.states {
            match state_index(description, state) {
                Ok(index) => {
                    let expected = description.base_state_id as u32 + index;
                    if state.id != expected {
                        mismatches.push(format!(
                            "{}: state {} should be {expected}",
                            description.identifier, state.id
                        ));
                    }
                }
                Err(error) => mismatches.push(format!("{}: {error}", description.identifier)),
            }
        }
    }
    assert!(seen > 0, "no block was compared");
    assert_no_mismatches("states with an unexpected id", mismatches);
}

#[test]
fn the_reported_default_state_is_the_definitions_default() {
    let corpus = corpus();
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for description in descriptions() {
        let Some(block) = reported(description, &corpus.report) else {
            continue;
        };
        seen += 1;
        let defaults: Vec<u32> = block
            .states
            .iter()
            .filter(|state| state.default)
            .map(|state| state.id)
            .collect();
        if defaults != [description.default_state_id as u32] {
            mismatches.push(format!(
                "{}: reported defaults {defaults:?}, definition {}",
                description.identifier, description.default_state_id
            ));
        }
    }
    assert!(seen > 0, "no block was compared");
    assert_no_mismatches("blocks with a different default state", mismatches);
}

#[test]
fn the_reported_state_count_is_the_product_of_the_property_value_counts() {
    let corpus = corpus();
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for description in descriptions() {
        let Some(block) = reported(description, &corpus.report) else {
            continue;
        };
        seen += 1;
        let expected = description.properties.state_count();
        if block.states.len() != expected {
            mismatches.push(format!(
                "{}: {} reported states, definition {expected}",
                description.identifier,
                block.states.len()
            ));
        }
        if description.properties.0.is_empty()
            && block
                .states
                .iter()
                .map(|state| state.id)
                .collect::<Vec<_>>()
                != [description.base_state_id as u32]
        {
            mismatches.push(format!(
                "{}: a block without properties is one state at {}",
                description.identifier, description.base_state_id
            ));
        }
    }
    assert!(seen > 0, "no block was compared");
    assert!(
        descriptions().any(|description| description.properties.0.is_empty()),
        "no block without properties was seen"
    );
    assert_no_mismatches("blocks with a different state count", mismatches);
}

#[test]
fn definitions_and_the_report_name_the_same_blocks() {
    let corpus = corpus();
    let defined: BTreeSet<&str> = descriptions()
        .map(|description| description.identifier.as_str())
        .collect();
    assert_eq!(
        defined.len(),
        corpus.definitions.len(),
        "an identifier is defined twice"
    );
    let reported: BTreeSet<&str> = corpus.report.keys().map(String::as_str).collect();
    assert!(!defined.is_empty());
    let mut mismatches: Vec<String> = defined
        .difference(&reported)
        .map(|name| format!("{name}: defined, not in the report"))
        .collect();
    mismatches.extend(
        reported
            .difference(&defined)
            .map(|name| format!("{name}: in the report, not defined")),
    );
    assert_no_mismatches("blocks missing on one side", mismatches);
}

#[test]
fn the_reported_states_tile_the_state_space() {
    let corpus = corpus();
    let mut claimed: BTreeMap<u32, &str> = BTreeMap::new();
    let mut mismatches = Vec::new();
    for (name, block) in &corpus.report {
        for state in &block.states {
            if let Some(other) = claimed.insert(state.id, name) {
                mismatches.push(format!(
                    "state {} is claimed by {other} and {name}",
                    state.id
                ));
            }
        }
    }
    assert!(!claimed.is_empty(), "the report lists no state");
    for expected in 0..claimed.len() as u32 {
        if !claimed.contains_key(&expected) {
            mismatches.push(format!("state {expected} is claimed by no block"));
        }
    }
    assert_no_mismatches("gaps or overlaps among the reported states", mismatches);
}

#[test]
fn block_protocol_ids_match_the_registries_report() {
    let corpus = corpus();
    let blocks = corpus
        .registries
        .registry("block")
        .expect("the registries report has no block registry");
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for description in descriptions() {
        seen += 1;
        let registered = blocks.names().get(description.protocol_id as usize);
        if registered != Some(&description.identifier) {
            mismatches.push(format!(
                "{}: protocol_id {} is {registered:?} in the registries report",
                description.identifier, description.protocol_id
            ));
        }
    }
    assert!(seen > 0, "no block definition was read");
    assert_no_mismatches("blocks with a different protocol id", mismatches);
}
