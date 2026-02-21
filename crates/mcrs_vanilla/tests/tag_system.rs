//! Integration test for the tag system: StaticRegistry + StaticTags + freeze + IdBitSet.
//!
//! Exercises the full lifecycle without any network, player, or Bevy asset pipeline:
//!   1. Populate StaticRegistry<Block> and StaticRegistry<Item>
//!   2. Insert resolved tags (simulating what resolve_*_tags does)
//!   3. Verify contains() works before freeze (HashSet path)
//!   4. Freeze to bitset storage
//!   5. Verify contains() works after freeze (bitset path)
//!   6. Verify get() returns correct IdBitSet with correct membership
//!   7. Verify iter() yields all tags
//!   8. Verify Tool::get_mining_speed and is_correct_block_for_drops

use mcrs_core::{StaticId, StaticRegistry, StaticTags};
use mcrs_vanilla::block::tags as block_tags;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::item::tags as item_tags;
use mcrs_vanilla::item::Item;
use std::collections::HashSet;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn build_block_registry() -> StaticRegistry<Block> {
    let mut reg = StaticRegistry::new();
    mcrs_vanilla::block::minecraft::register_all_blocks(&mut reg);
    reg
}

fn build_item_registry() -> StaticRegistry<Item> {
    let mut reg = StaticRegistry::new();
    mcrs_vanilla::item::minecraft::register_all_items(&mut reg);
    reg
}

/// Simulate tag resolution: given a list of block names, build a HashSet<StaticId<Block>>.
fn resolve_block_ids(registry: &StaticRegistry<Block>, names: &[&str]) -> HashSet<StaticId<Block>> {
    let mut set = HashSet::new();
    for name in names {
        if let Some(id) = registry.id_of(name) {
            set.insert(id);
        }
    }
    set
}

fn resolve_item_ids(registry: &StaticRegistry<Item>, names: &[&str]) -> HashSet<StaticId<Item>> {
    let mut set = HashSet::new();
    for name in names {
        if let Some(id) = registry.id_of(name) {
            set.insert(id);
        }
    }
    set
}

// ─── Registry smoke tests ───────────────────────────────────────────────────

#[test]
fn block_registry_populates() {
    let reg = build_block_registry();
    assert!(reg.len() > 0, "block registry should not be empty");
    assert!(
        reg.id_of("minecraft:stone").is_some(),
        "stone should be registered"
    );
    assert!(
        reg.id_of("minecraft:air").is_some(),
        "air should be registered"
    );
    assert!(
        reg.id_of("minecraft:diamond_ore").is_some(),
        "diamond_ore should be registered"
    );
    assert!(
        reg.id_of("minecraft:nonexistent").is_none(),
        "nonexistent should not be registered"
    );
}

#[test]
fn item_registry_populates() {
    let reg = build_item_registry();
    assert!(reg.len() > 0, "item registry should not be empty");
    assert!(
        reg.id_of("minecraft:wooden_pickaxe").is_some(),
        "wooden_pickaxe should be registered"
    );
    assert!(
        reg.id_of("minecraft:diamond_pickaxe").is_some(),
        "diamond_pickaxe should be registered"
    );
}

#[test]
fn registry_ids_are_dense() {
    let reg = build_block_registry();
    // Verify every registered entry can be retrieved by its id.
    for (id, _loc, _block) in reg.iter() {
        assert!(
            reg.get_by_id(id).is_some(),
            "StaticId({}) should map to a block",
            id.raw()
        );
    }
    // Verify the iter count matches len.
    assert_eq!(reg.iter().count(), reg.len());
}

// ─── Tag lifecycle: insert → contains (pre-freeze) → freeze → contains (post-freeze) ──

#[test]
fn block_tag_lifecycle() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    // Simulate resolved "mineable/pickaxe" tag with a subset of blocks.
    let pickaxe_blocks = [
        "minecraft:stone",
        "minecraft:granite",
        "minecraft:cobblestone",
    ];
    let ids = resolve_block_ids(&block_reg, &pickaxe_blocks);
    assert_eq!(ids.len(), pickaxe_blocks.len());

    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        ids,
    );

    // Pre-freeze: contains() should work via HashSet fallback.
    let stone_id = block_reg.id_of("minecraft:stone").unwrap();
    let granite_id = block_reg.id_of("minecraft:granite").unwrap();
    let cobblestone_id = block_reg.id_of("minecraft:cobblestone").unwrap();
    let air_id = block_reg.id_of("minecraft:air").unwrap();

    assert!(!tags.is_frozen());
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, stone_id));
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, granite_id));
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, cobblestone_id));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, air_id));

    // Freeze.
    tags.freeze(block_reg.len() as u32);
    assert!(tags.is_frozen());

    // Post-freeze: contains() should work via bitset.
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, stone_id));
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, granite_id));
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, cobblestone_id));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, air_id));
}

#[test]
fn item_tag_lifecycle() {
    let item_reg = build_item_registry();
    let mut tags = StaticTags::<Item>::new();

    let pickaxe_items = [
        "minecraft:wooden_pickaxe",
        "minecraft:stone_pickaxe",
        "minecraft:iron_pickaxe",
        "minecraft:golden_pickaxe",
        "minecraft:diamond_pickaxe",
    ];
    let ids = resolve_item_ids(&item_reg, &pickaxe_items);
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:pickaxes").unwrap(),
        ids,
    );

    tags.freeze(item_reg.len() as u32);

    for name in &pickaxe_items {
        let id = item_reg.id_of(name).unwrap();
        assert!(
            tags.contains(&item_tags::PICKAXES, id),
            "{name} should be in pickaxes tag"
        );
    }
}

// ─── get() returns IdBitSet after freeze ────────────────────────────────────

#[test]
fn get_returns_bitset_after_freeze() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    let log_blocks = ["minecraft:oak_planks"]; // using a known registered block
    let ids = resolve_block_ids(&block_reg, &log_blocks);
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:logs").unwrap(),
        ids,
    );

    // get() returns None before freeze.
    assert!(tags.get(&block_tags::LOGS).is_none());

    tags.freeze(block_reg.len() as u32);

    let bs = tags.get(&block_tags::LOGS).expect("logs tag should exist");
    assert_eq!(bs.len(), 1);

    let oak_id = block_reg.id_of("minecraft:oak_planks").unwrap();
    assert!(bs.contains(oak_id));

    let stone_id = block_reg.id_of("minecraft:stone").unwrap();
    assert!(!bs.contains(stone_id));
}

// ─── Multiple tags are independent ──────────────────────────────────────────

#[test]
fn multiple_block_tags_independent() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    // mineable/pickaxe: stone, granite
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:stone", "minecraft:granite"]),
    );

    // mineable/shovel: dirt, grass_block
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/shovel").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:dirt", "minecraft:grass_block"]),
    );

    // sand: just check with a known block
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:sand").unwrap(),
        HashSet::new(), // empty tag
    );

    tags.freeze(block_reg.len() as u32);

    let stone_id = block_reg.id_of("minecraft:stone").unwrap();
    let dirt_id = block_reg.id_of("minecraft:dirt").unwrap();
    let granite_id = block_reg.id_of("minecraft:granite").unwrap();
    let grass_id = block_reg.id_of("minecraft:grass_block").unwrap();

    // Pickaxe tags
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, stone_id));
    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, granite_id));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, dirt_id));

    // Shovel tags
    assert!(tags.contains(&block_tags::MINEABLE_SHOVEL, dirt_id));
    assert!(tags.contains(&block_tags::MINEABLE_SHOVEL, grass_id));
    assert!(!tags.contains(&block_tags::MINEABLE_SHOVEL, stone_id));

    // Sand tag (empty)
    assert!(!tags.contains(&block_tags::SAND, stone_id));
}

// ─── iter() yields all frozen tags ──────────────────────────────────────────

#[test]
fn iter_yields_all_frozen_tags() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:stone"]),
    );
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/axe").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:oak_planks"]),
    );
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:leaves").unwrap(),
        HashSet::new(),
    );

    tags.freeze(block_reg.len() as u32);

    let mut tag_names: Vec<String> = tags.iter().map(|(rl, _)| rl.as_str().to_string()).collect();
    tag_names.sort();
    assert_eq!(
        tag_names,
        vec![
            "minecraft:leaves",
            "minecraft:mineable/axe",
            "minecraft:mineable/pickaxe",
        ]
    );
}

// ─── Pre-freeze / post-freeze consistency ───────────────────────────────────

#[test]
fn contains_consistent_before_and_after_freeze() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    // Insert a tag with several blocks.
    let mineable: Vec<&str> = vec![
        "minecraft:stone",
        "minecraft:granite",
        "minecraft:diorite",
        "minecraft:andesite",
        "minecraft:cobblestone",
        "minecraft:diamond_ore",
    ];
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(&block_reg, &mineable),
    );

    // Snapshot pre-freeze results for all registered blocks.
    let all_ids: Vec<StaticId<Block>> = block_reg.iter().map(|(id, _, _)| id).collect();
    let pre_freeze: Vec<(StaticId<Block>, bool)> = all_ids
        .iter()
        .map(|&id| (id, tags.contains(&block_tags::MINEABLE_PICKAXE, id)))
        .collect();

    tags.freeze(block_reg.len() as u32);

    // Verify post-freeze matches pre-freeze exactly.
    for (id, expected) in &pre_freeze {
        assert_eq!(
            tags.contains(&block_tags::MINEABLE_PICKAXE, *id),
            *expected,
            "mismatch at StaticId({})",
            id.raw()
        );
    }
}

// ─── Panic guards ───────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "freeze() called twice")]
fn double_freeze_panics() {
    let mut tags = StaticTags::<Block>::new();
    tags.freeze(64);
    tags.freeze(64);
}

#[test]
#[should_panic(expected = "insert() called after freeze()")]
fn insert_after_freeze_panics() {
    let mut tags = StaticTags::<Block>::new();
    tags.freeze(64);
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:test").unwrap(),
        HashSet::new(),
    );
}

// ─── Tool mining speed with frozen tags ─────────────────────────────────────

#[test]
fn tool_mining_speed_uses_frozen_tags() {
    let block_reg = build_block_registry();
    let item_reg = build_item_registry();
    let mut block_tags_res = StaticTags::<Block>::new();

    // Stone is mineable by pickaxe.
    block_tags_res.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(
            &block_reg,
            &[
                "minecraft:stone",
                "minecraft:granite",
                "minecraft:diamond_ore",
            ],
        ),
    );

    // incorrect_for_wooden_tool includes diamond_ore (simulating vanilla behavior).
    block_tags_res.insert(
        mcrs_core::ResourceLocation::parse("minecraft:incorrect_for_wooden_tool").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:diamond_ore"]),
    );

    block_tags_res.freeze(block_reg.len() as u32);

    // Get the wooden pickaxe's Tool component.
    let wooden_item = item_reg.get_by_loc("minecraft:wooden_pickaxe").unwrap();
    let tool = wooden_item
        .components
        .tool
        .as_ref()
        .expect("wooden_pickaxe should have a tool component");

    // Stone is mineable by wooden pickaxe — should get boosted speed.
    let stone_block = block_reg.get_by_loc("minecraft:stone").unwrap();
    let speed = tool.get_mining_speed(stone_block, &block_reg, &block_tags_res);
    assert!(
        speed > 1.0,
        "wooden pickaxe should mine stone faster than default (got {speed})"
    );
    assert!(
        (speed - 2.0).abs() < 0.001,
        "wooden pickaxe speed for stone should be 2.0 (WOOD material speed), got {speed}"
    );

    // Air is not mineable by pickaxe — should get default speed.
    let air_block = block_reg.get_by_loc("minecraft:air").unwrap();
    let air_speed = tool.get_mining_speed(air_block, &block_reg, &block_tags_res);
    assert!(
        (air_speed - 2.0).abs() < 0.001,
        "default mining speed should be 2.0 (WOOD material speed), got {air_speed}"
    );
}

#[test]
fn tool_correct_for_drops_with_frozen_tags() {
    let block_reg = build_block_registry();
    let item_reg = build_item_registry();
    let mut block_tags_res = StaticTags::<Block>::new();

    // Stone is mineable by pickaxe.
    block_tags_res.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:stone", "minecraft:diamond_ore"]),
    );

    // Diamond ore is incorrect for wooden tool (requires better material).
    block_tags_res.insert(
        mcrs_core::ResourceLocation::parse("minecraft:incorrect_for_wooden_tool").unwrap(),
        resolve_block_ids(&block_reg, &["minecraft:diamond_ore"]),
    );

    block_tags_res.freeze(block_reg.len() as u32);

    let wooden_item = item_reg.get_by_loc("minecraft:wooden_pickaxe").unwrap();
    let tool = wooden_item.components.tool.as_ref().unwrap();

    // Stone: wooden pickaxe IS correct for drops (mineable/pickaxe matches,
    // incorrect_for_wooden_tool does NOT include stone → denies_drops rule doesn't match).
    let stone_block = block_reg.get_by_loc("minecraft:stone").unwrap();
    assert!(
        tool.is_correct_block_for_drops(stone_block, &block_reg, &block_tags_res),
        "wooden pickaxe should be correct for stone drops"
    );

    // Diamond ore: wooden pickaxe is NOT correct for drops (incorrect_for_wooden_tool
    // includes diamond_ore → denies_drops rule matches first).
    let diamond_ore = block_reg.get_by_loc("minecraft:diamond_ore").unwrap();
    assert!(
        !tool.is_correct_block_for_drops(diamond_ore, &block_reg, &block_tags_res),
        "wooden pickaxe should NOT be correct for diamond_ore drops"
    );

    // Iron pickaxe should be correct for diamond_ore (it uses incorrect_for_iron_tool,
    // which we didn't populate, so the denies_drops rule won't match).
    let iron_item = item_reg.get_by_loc("minecraft:iron_pickaxe").unwrap();
    let iron_tool = iron_item.components.tool.as_ref().unwrap();
    assert!(
        iron_tool.is_correct_block_for_drops(diamond_ore, &block_reg, &block_tags_res),
        "iron pickaxe should be correct for diamond_ore drops"
    );
}

// ─── ALL_BLOCK_TAGS / ALL_ITEM_TAGS arrays ──────────────────────────────────

#[test]
fn all_block_tags_contains_expected_entries() {
    let has = |needle: &str| -> bool {
        block_tags::ALL_BLOCK_TAGS
            .iter()
            .any(|t| t.resource_location().as_static_str() == needle)
    };

    assert!(
        has("minecraft:mineable/pickaxe"),
        "missing mineable/pickaxe"
    );
    assert!(has("minecraft:mineable/axe"), "missing mineable/axe");
    assert!(has("minecraft:mineable/shovel"), "missing mineable/shovel");
    assert!(has("minecraft:mineable/hoe"), "missing mineable/hoe");
    assert!(
        has("minecraft:needs_correct_tool_for_drops"),
        "missing needs_correct_tool"
    );
    assert!(
        has("minecraft:incorrect_for_wooden_tool"),
        "missing incorrect_for_wooden_tool"
    );
    assert!(
        has("minecraft:incorrect_for_diamond_tool"),
        "missing incorrect_for_diamond_tool"
    );
    assert!(has("minecraft:logs"), "missing logs");
    assert!(has("minecraft:leaves"), "missing leaves");
    assert_eq!(
        block_tags::ALL_BLOCK_TAGS.len(),
        17,
        "expected 17 block tags"
    );
}

#[test]
fn all_item_tags_contains_expected_entries() {
    let has = |needle: &str| -> bool {
        item_tags::ALL_ITEM_TAGS
            .iter()
            .any(|t| t.resource_location().as_static_str() == needle)
    };

    assert!(has("minecraft:swords"), "missing swords");
    assert!(has("minecraft:pickaxes"), "missing pickaxes");
    assert!(has("minecraft:axes"), "missing axes");
    assert!(has("minecraft:shovels"), "missing shovels");
    assert!(has("minecraft:hoes"), "missing hoes");
    assert_eq!(item_tags::ALL_ITEM_TAGS.len(), 5, "expected 5 item tags");
}

// ─── Bulk tag request simulation ────────────────────────────────────────────

#[test]
fn all_block_tags_can_be_inserted_and_frozen() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    // Insert an empty set for every tag in ALL_BLOCK_TAGS.
    for tag_key in block_tags::ALL_BLOCK_TAGS {
        tags.insert(tag_key.resource_location().to_arc(), HashSet::new());
    }

    tags.freeze(block_reg.len() as u32);
    assert!(tags.is_frozen());

    // Every tag should be accessible via get().
    for tag_key in block_tags::ALL_BLOCK_TAGS {
        assert!(
            tags.get(tag_key).is_some(),
            "tag {} should exist after freeze",
            tag_key.resource_location().as_str()
        );
    }
}

// ─── IdBitSet iterator agreement with contains ──────────────────────────────

#[test]
fn bitset_iter_matches_contains() {
    let block_reg = build_block_registry();
    let mut tags = StaticTags::<Block>::new();

    let blocks = [
        "minecraft:stone",
        "minecraft:granite",
        "minecraft:diorite",
        "minecraft:cobblestone",
        "minecraft:diamond_ore",
    ];
    tags.insert(
        mcrs_core::ResourceLocation::parse("minecraft:mineable/pickaxe").unwrap(),
        resolve_block_ids(&block_reg, &blocks),
    );

    tags.freeze(block_reg.len() as u32);

    let bs = tags.get(&block_tags::MINEABLE_PICKAXE).unwrap();
    assert_eq!(bs.len() as usize, blocks.len());

    // Every ID yielded by iter() should be contains()==true.
    let iter_ids: Vec<StaticId<Block>> = bs.iter().collect();
    assert_eq!(iter_ids.len(), blocks.len());

    for id in &iter_ids {
        assert!(bs.contains(*id), "iter yielded id that contains() rejects");
    }

    // And those IDs should correspond to the expected blocks.
    let expected_ids: HashSet<u32> = blocks
        .iter()
        .map(|name| block_reg.id_of(name).unwrap().raw())
        .collect();
    let actual_ids: HashSet<u32> = iter_ids.iter().map(|id| id.raw()).collect();
    assert_eq!(expected_ids, actual_ids);
}
