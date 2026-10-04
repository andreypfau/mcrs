use mcrs_minecraft_block::definition::schema::BlockDefinitionFile;
use mcrs_minecraft_registry::static_report::from_report;
use std::path::{Path, PathBuf};

use crate::common::assert_no_mismatches;

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
