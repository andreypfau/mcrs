use std::sync::LazyLock;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::build_block_definitions;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_falling_block::ConcretePairsFile;
use mcrs_minecraft_falling_block::ConcreteTable;
use mcrs_minecraft_falling_block::FallingBlockTable;
use mcrs_minecraft_falling_block::PackConcretePairs;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_voxel::BlockStateId;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/");

pub static DEFINITIONS: LazyLock<BlockDefinitions> = LazyLock::new(|| {
    let directory = format!("{ASSETS}mcrs/block_definition");
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
});

pub static DEFAULT_PAIRS: LazyLock<ConcretePairsFile> = LazyLock::new(|| {
    let path = format!(
        "{ASSETS}mcrs/datapacks/mcrs/{}",
        mcrs_minecraft_falling_block::CONCRETE_PAIRS_PATH
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
});

pub static TABLE: LazyLock<FallingBlockTable> = LazyLock::new(|| {
    let registries = test_registries();
    let concrete = ConcreteTable::load(
        &[PackConcretePairs {
            pack: "mcrs".to_owned(),
            file: DEFAULT_PAIRS.clone(),
        }],
        &DEFINITIONS,
    )
    .unwrap();
    FallingBlockTable::new(
        &DEFINITIONS,
        &registries.tags::<Block>().unwrap(),
        &registries.tags::<Fluid>().unwrap(),
        &concrete,
    )
});

pub fn state(block: &str, properties: &[(&str, &str)]) -> BlockStateId {
    let entry = DEFINITIONS.block(block).expect(block);
    properties
        .iter()
        .fold(entry.default_state_id, |state, (property, value)| {
            entry.with_text(state, property, value).expect(property)
        })
}
