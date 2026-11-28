#![expect(
    clippy::expect_used,
    reason = "test support reports a failure by panicking"
)]

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{
    Blocks, CORPUS_DIRECTORY as BLOCK_CORPUS, build_block_definitions,
};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY as ITEM_CORPUS;

use crate::item_definitions::items_from_files;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_table::ItemTable;

pub fn test_corpus() -> &'static (Blocks, Items) {
    &corpus().0
}

pub fn test_item_table() -> ItemTable {
    corpus().1
}

fn corpus() -> &'static ((Blocks, Items), ItemTable) {
    static CORPUS: OnceLock<((Blocks, Items), ItemTable)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let registries = test_registries();
        let block_names = Block::ALL
            .iter()
            .map(|entry| entry.as_static_str())
            .collect::<Vec<_>>();
        let (blocks, _) = build_block_definitions(&block_names, read_json_directory(BLOCK_CORPUS))
            .expect("the block corpus loads");
        let items = items_from_files(read_json_directory(ITEM_CORPUS), registries, &blocks)
            .expect("the item corpus loads");
        (
            (Blocks(Arc::new(blocks)), Items(Arc::new(items.definitions))),
            items.table,
        )
    })
}

fn read_json_directory(directory: &str) -> Vec<(String, Vec<u8>)> {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets")).join(directory);
    let mut names: Vec<OsString> = fs::read_dir(&root)
        .expect("the corpus directory is readable")
        .map(|entry| {
            entry
                .expect("the corpus directory lists its entries")
                .file_name()
        })
        .filter(|name| !name.to_string_lossy().starts_with('.'))
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let bytes = fs::read(root.join(&name)).expect("the corpus file is readable");
            (
                Path::new(directory).join(&name).display().to_string(),
                bytes,
            )
        })
        .collect()
}
