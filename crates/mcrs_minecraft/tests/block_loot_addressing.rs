mod support;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use mcrs_vanilla::block::definition::BlockDefinitions;

fn corpus() -> &'static BlockDefinitions {
    support::standalone_corpus()
}

fn table_of(blocks: &BlockDefinitions, block: &str) -> Option<String> {
    let state = blocks.default_state(block);
    blocks
        .state(state)
        .loot
        .map(|loot| blocks.loot_table(loot).as_str().to_owned())
}

/// The corpus names the table; nothing guesses it from the block's own name.
#[test]
fn the_corpus_names_the_loot_table() {
    let blocks = corpus();
    assert_eq!(
        table_of(blocks, "minecraft:stone").as_deref(),
        Some("minecraft:blocks/stone")
    );
}

#[test]
fn a_block_with_no_table_has_no_loot_id() {
    let blocks = corpus();
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
}

/// Two blocks, one table. Keying by the table rather than by the block is what
/// makes that a single entry instead of two copies.
#[test]
fn blocks_that_share_a_table_share_one_loot_id() {
    let blocks = corpus();
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

/// The warm-up walks the interned tables, not the blocks, so a shared table is
/// requested once and a block without one is never requested at all.
#[test]
fn the_corpus_interns_fewer_tables_than_it_has_blocks() {
    let blocks = corpus();
    assert_eq!(blocks.blocks().len(), 1286);
    assert_eq!(blocks.loot_table_count(), 1201);
}

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
        let table = blocks.loot_table(mcrs_vanilla::block::definition::LootId(index as u16));
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

/// The dimension sub-app's warm-up is what fills `BlockLootTables`; this checks
/// the resource the plugin installs is keyed by the corpus' loot id.
#[test]
fn block_loot_tables_is_keyed_by_loot_id() {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.insert_resource(support::corpus(&app));
    app.add_plugins(mcrs_minecraft::world::loot::LootPlugin);

    let blocks = app
        .world()
        .resource::<mcrs_vanilla::block::definition::Blocks>()
        .clone();
    let stone = blocks.state(blocks.default_state("minecraft:stone")).loot;
    let tables = app
        .world()
        .resource::<mcrs_minecraft::world::loot::BlockLootTables>();
    assert!(tables.tables.is_empty(), "nothing is resolved before load");
    assert!(stone.is_some());
}
