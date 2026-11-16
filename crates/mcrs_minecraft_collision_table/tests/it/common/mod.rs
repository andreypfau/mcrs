//! The corpus loaders of the tests of this crate.
#![expect(
    clippy::unwrap_used,
    reason = "the corpus loaders fail the test by panicking"
)]

use std::sync::OnceLock;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::build_block_definitions;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_registries::Tags;

pub fn definitions() -> &'static BlockDefinitions {
    static DEFINITIONS: OnceLock<BlockDefinitions> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let directory = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/block_definition"
        );
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .map(|path| (path.display().to_string(), std::fs::read(&path).unwrap()))
            .collect();
        files.sort();
        let names: Vec<&str> = Block::ALL
            .iter()
            .map(|block| block.as_static_str())
            .collect();
        build_block_definitions(&names, files).unwrap().0
    })
}

pub fn tags() -> &'static Tags<Block> {
    static TAGS: OnceLock<Tags<Block>> = OnceLock::new();
    TAGS.get_or_init(|| {
        mcrs_minecraft_game_registries::registry_load::test_registries()
            .tags::<Block>()
            .unwrap()
    })
}

pub fn table() -> &'static CollisionTable {
    static TABLE: OnceLock<CollisionTable> = OnceLock::new();
    TABLE.get_or_init(|| collision_table(definitions(), tags()))
}
