use std::sync::LazyLock;

use mcrs_minecraft_block::definition::LootId;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::{ResourceKey, rl};
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_loot::LootCondition;
use mcrs_minecraft_protocol::item::Enchantments;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_server::world::loot::condition::check;
use mcrs_minecraft_server::world::loot::context::BlockBreakContext;
use mcrs_minecraft_server::world::loot::entry::roll;
use mcrs_minecraft_server::world::loot::{BlockLootTables, LootRegistries};
use mcrs_minecraft_world::registries::test_registries;

static LOOT: LazyLock<LootRegistries> =
    LazyLock::new(|| LootRegistries::from_set(test_registries()));

fn silk_touch() -> Enchantments {
    Enchantments(vec![(
        ResourceKey::from_location(rl!("minecraft:silk_touch").to_arc()),
        1,
    )])
}

fn drops(table: &str, state: BlockStateId, tool_enchantments: Option<&Enchantments>) -> Vec<Item> {
    let blocks = &crate::support::standalone_corpus().0;
    let tags = test_registries().tags::<Block>().unwrap();
    let id = LOOT.tables.by_name(table).unwrap();
    let ctx = BlockBreakContext {
        loot: &LOOT,
        blocks,
        state,
        tags: &tags,
        tool_enchantments,
    };
    roll(&LOOT.bodies[id], &ctx)
        .into_iter()
        .map(|drop| {
            assert_eq!(drop.count, 1);
            drop.item
        })
        .collect()
}

/// Conditions come from the `condition` field, written in place or named from
/// the predicate registry: only the lower half of a door drops it, and glass
/// needs silk touch, which `minecraft:tool/can_silk_touch` names.
#[test]
fn block_loot_follows_its_conditions() {
    let blocks = &crate::support::standalone_corpus().0;
    let door = blocks.block("minecraft:oak_door").unwrap();
    let half = |half| door.with_text(door.default_state_id, "half", half).unwrap();
    let glass = blocks.default_state("minecraft:glass");
    let silk = silk_touch();

    let cases = [
        (
            "minecraft:blocks/oak_door",
            half("lower"),
            None,
            vec![Item::OakDoor],
        ),
        ("minecraft:blocks/oak_door", half("upper"), None, vec![]),
        ("minecraft:blocks/glass", glass, None, vec![]),
        (
            "minecraft:blocks/glass",
            glass,
            Some(&silk),
            vec![Item::Glass],
        ),
    ];
    for (table, state, tool_enchantments, expected) in cases {
        assert_eq!(
            drops(table, state, tool_enchantments),
            expected,
            "{table} state {state:?}"
        );
    }
}

#[test]
fn a_match_block_condition_tests_a_block_tag_by_membership() {
    let registries = test_registries();
    let tags = registries.tags::<Block>().unwrap();
    let blocks = &crate::support::standalone_corpus().0;
    let condition: LootCondition = registries.scope(|| {
        serde_json::from_str(
            r##"{"type":"minecraft:match_block","blocks":"#minecraft:mineable/pickaxe"}"##,
        )
        .unwrap()
    });
    let holds = |block: &str| {
        check(
            &condition,
            &BlockBreakContext {
                loot: &LOOT,
                blocks,
                state: blocks.default_state(block),
                tags: &tags,
                tool_enchantments: None,
            },
        )
    };
    assert!(holds("minecraft:stone"));
    assert!(!holds("minecraft:dirt"));
}

#[test]
fn every_loot_table_the_block_corpus_names_is_loaded() {
    let blocks = &crate::support::standalone_corpus().0;
    let tables = BlockLootTables::new(blocks, &LOOT.tables);
    let missing: Vec<_> = (0..blocks.loot_table_count())
        .map(|index| LootId(index as u16))
        .filter(|&loot| tables.get(loot).is_none())
        .map(|loot| blocks.loot_table(loot).to_string())
        .collect();
    assert!(blocks.loot_table_count() > 1000);
    assert_eq!(missing, Vec::<String>::new());
}
