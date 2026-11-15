use glam::DVec3;
use mcrs_minecraft_item_stack::{Count, ItemStack};
use mcrs_minecraft_loot_condition::Tool;
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_test_alloc::allocations;

use crate::condition::{condition, empty_context, item};
use crate::number_provider::{VANILLA, inputs};

/// LOOT-CND-007, LOOT-CND-008, LOOT-CND-009: a test that reads no saved data allocates nothing
/// after the scratch has warmed up.
#[test]
fn loot_cnd_a_test_without_saved_data_allocates_nothing_after_warm_up() {
    let tree = condition(
        r#"{"type": "minecraft:all_of", "terms": [
            {"type": "minecraft:match_tool", "predicate": {"items": "minecraft:diamond_pickaxe"}},
            {"type": "minecraft:location_check", "offsetY": -1, "predicate": {"position": {"y": {"min": 0.0}}}},
            {"type": "minecraft:entity_properties", "entity": "this", "predicate": {}}
        ]}"#,
    );
    let pickaxe = ItemStack::new(
        item("minecraft:diamond_pickaxe"),
        Count::new(1).expect("a count of 1"),
    );
    let mut context = empty_context();
    context.tool = Some(Tool::Stack(&pickaxe));
    context.origin = Some(DVec3::new(0.5, 64.5, 0.5));
    context.this_entity = Some(());
    let mut random = XoroshiroRandom::new(42);
    let mut scratch = PredicateScratch::default();
    let inputs = inputs(&VANILLA, &context);
    assert!(tree.test(inputs, &mut random, &mut scratch));
    let before = allocations();
    let passed = tree.test(inputs, &mut random, &mut scratch);
    assert_eq!((passed, allocations() - before), (true, 0));
}
