use crate::support;

use mcrs_minecraft_block::definition::BlockDefinitions;

fn corpus() -> &'static BlockDefinitions {
    &support::standalone_corpus().0.0
}

fn table_of(blocks: &BlockDefinitions, block: &str) -> Option<String> {
    let state = blocks.default_state(block);
    blocks
        .state(state)
        .loot
        .map(|loot| blocks.loot_table(loot).as_str().to_owned())
}

/// The corpus names the table; nothing guesses it from the block's own name.
/// Two blocks may share one table, and keying by the table rather than by the
/// block is what makes that a single entry instead of two copies.
#[test]
fn the_corpus_names_each_loot_table_once() {
    let blocks = corpus();
    assert_eq!(
        table_of(blocks, "minecraft:stone").as_deref(),
        Some("minecraft:blocks/stone")
    );

    for block in [
        "minecraft:air",
        "minecraft:cave_air",
        "minecraft:void_air",
        "minecraft:water",
        "minecraft:lava",
        "minecraft:bedrock",
    ] {
        assert_eq!(table_of(blocks, block), None, "{block} states a loot table");
    }

    let standing = blocks.default_state("minecraft:zombie_head");
    let wall = blocks.default_state("minecraft:zombie_wall_head");
    let standing_loot = blocks.state(standing).loot.expect("zombie head drops");
    let wall_loot = blocks.state(wall).loot.expect("zombie wall head drops");
    assert_eq!(standing_loot, wall_loot);
    assert_eq!(
        blocks.loot_table(standing_loot).as_str(),
        "minecraft:blocks/zombie_head"
    );
}

mod exhaustive {
    use super::*;

    /// Every interned table resolves to an asset path that exists, which is what
    /// the warm-up asks the asset server for.
    #[test]
    fn every_interned_table_names_an_asset_that_exists() {
        let blocks = corpus();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets");
        let mut missing = Vec::new();
        for index in 0..blocks.loot_table_count() {
            let table = blocks.loot_table(mcrs_minecraft_block::definition::LootId(index as u16));
            let path = root.join(format!(
                "{}/loot_table/{}.json",
                table.namespace(),
                table.path()
            ));
            if !path.exists() {
                missing.push(table.as_str().to_owned());
            }
        }
        assert!(missing.is_empty(), "missing loot table assets: {missing:?}");
    }
}
