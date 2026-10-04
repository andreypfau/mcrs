use mcrs_minecraft_block::definition::schema::BlockDefinitionFile;
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{BlockStateId, Registry};
use std::path::{Path, PathBuf};

use crate::common::{assert_no_mismatches, load_corpus, report_blocks};

#[test]
fn block_protocol_ids_match_the_registries_report() {
    let registries_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/reports/registries.json");
    let registries = from_report(
        &std::fs::read(&registries_path)
            .unwrap_or_else(|e| panic!("{}: {e}", registries_path.display())),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", registries_path.display()));
    let definitions: Vec<(PathBuf, BlockDefinitionFile)> = crate::common::definition_files();
    let blocks = registries
        .table("minecraft:block")
        .expect("the registries report has no block registry");
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for (_, file) in &definitions {
        let description = &file.block.description;
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

#[test]
fn a_blocks_table_index_is_its_report_id() {
    let registry = report_blocks();
    let (definitions, _) = load_corpus(&registry).expect("the corpus loads");
    let blocks = definitions.blocks();
    assert_eq!(blocks.len(), registry.len());
    let mut mismatches = Vec::new();
    for id in registry.ids() {
        let name = registry.key(id).expect("every id has a name").as_str();
        if definitions.index_of(name) != Some(id.index() as u32)
            || blocks[id.index()].identifier.as_str() != name
            || usize::from(blocks[id.index()].protocol_id) != id.index()
        {
            mismatches.push(format!("{name} is not at report id {}", id.index()));
        }
    }
    assert_no_mismatches("blocks away from their report id", mismatches);
    assert_eq!(blocks[0].identifier.as_str(), "minecraft:air");
    let last = registry.len() - 1;
    assert_eq!(
        blocks[last].identifier.as_str(),
        registry
            .key(registry.ids().last().unwrap())
            .unwrap()
            .as_str()
    );
}

#[test]
fn touching_state_ranges_keep_their_owners() {
    let registry = report_blocks();
    let (definitions, _) = load_corpus(&registry).expect("the corpus loads");
    let mut by_state: Vec<_> = definitions.blocks().iter().collect();
    by_state.sort_by_key(|block| block.base_state_id);
    assert_eq!(by_state[0].base_state_id, BlockStateId(0));
    let mut mismatches = Vec::new();
    for pair in by_state.windows(2) {
        let (before, after) = (pair[0], pair[1]);
        let last_before = BlockStateId(after.base_state_id.0 - 1);
        if before.base_state_id.0 + before.state_count != after.base_state_id.0
            || definitions.owner(last_before).identifier != before.identifier
            || definitions.owner(after.base_state_id).identifier != after.identifier
        {
            mismatches.push(format!(
                "{} and {} do not touch cleanly",
                before.identifier, after.identifier
            ));
        }
    }
    assert_no_mismatches("neighbouring blocks", mismatches);
}

#[test]
fn a_single_state_block_owns_only_its_state() {
    let registry = report_blocks();
    let (definitions, _) = load_corpus(&registry).expect("the corpus loads");
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for block in definitions.blocks().iter().filter(|b| b.state_count == 1) {
        seen += 1;
        let state = block.base_state_id;
        let alone = block.default_state_id == state
            && block.owns(state)
            && !block.owns(BlockStateId(state.0.wrapping_add(1)))
            && state
                .0
                .checked_sub(1)
                .is_none_or(|before| !block.owns(BlockStateId(before)))
            && definitions.owner(state).identifier == block.identifier;
        if !alone {
            mismatches.push(format!("{} does not own only its state", block.identifier));
        }
    }
    assert!(seen > 0, "the corpus has no single-state block");
    assert_no_mismatches("single-state blocks", mismatches);
}

#[test]
fn a_definition_that_disagrees_with_the_report_is_refused() {
    let report = report_blocks();
    let mut names: Vec<_> = report
        .ids()
        .map(|id| report.key(id).unwrap().clone())
        .collect();
    names.swap(1, 2);
    let swapped = Registry::<Block>::new(names.clone(), std::iter::empty()).unwrap();
    let error = load_corpus(&swapped)
        .err()
        .expect("a swapped order is refused");
    let message = error.to_string();
    assert!(
        message.contains(names[1].as_str()) || message.contains(names[2].as_str()),
        "{message}"
    );
}
