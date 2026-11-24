use glam::DVec3;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::{Count, ItemStack};
use mcrs_minecraft_loot_condition::Tool;
use mcrs_minecraft_predicate::{BlockHost, DamageSource, PredicateScratch};
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_test_alloc::allocations;

use crate::condition::{condition, empty_context, item};
use crate::number_provider::{VANILLA, inputs};

#[test]
fn loot_cnd_a_test_without_saved_data_allocates_nothing_after_warm_up() {
    let tree = condition(
        r##"{"type": "minecraft:all_of", "terms": [
            {"type": "minecraft:match_tool", "predicate": {"items": "minecraft:diamond_pickaxe"}},
            {"type": "minecraft:location_check", "offsetY": -1, "predicate": {"position": {"y": {"min": 0.0}}}},
            {"type": "minecraft:entity_properties", "entity": "this", "predicate": {}},
            {"type": "minecraft:damage_source_properties", "predicate": {"tags": [{"id": "#minecraft:is_projectile", "expected": true}]}},
            {"type": "minecraft:time_check", "clock": "minecraft:overworld", "period": 24000, "value": {"max": 0}},
            {"type": "minecraft:inverted", "term": {"type": "minecraft:environment_attribute_check", "attribute": "minecraft:gameplay/bed_rule", "value": {"can_sleep": "never", "can_set_spawn": "never"}}}
        ]}"##,
    );
    let pickaxe = ItemStack::new(
        item("minecraft:diamond_pickaxe"),
        Count::new(1).expect("a count of 1"),
    );
    let arrow = test_registries()
        .entries::<DamageType>()
        .and_then(|entries| entries.by_name(ResourceLocation::read("minecraft:arrow").ok()?))
        .expect("a vanilla damage type");
    let mut context = empty_context();
    context.tool = Some(Tool::Stack(&pickaxe));
    context.origin = Some(DVec3::new(0.5, 64.5, 0.5));
    context.this_entity = Some(());
    context.damage_source = Some(DamageSource {
        damage_type: arrow,
        direct_entity: Some(()),
        causing_entity: None,
    });
    let mut random = XoroshiroRandom::new(42);
    let mut scratch = PredicateScratch::default();
    let world = BlockHost::none();
    let inputs = inputs(&VANILLA, &context, &world);
    assert!(tree.test(inputs, &mut random, &mut scratch));
    let before = allocations();
    let passed = tree.test(inputs, &mut random, &mut scratch);
    assert_eq!((passed, allocations() - before), (true, 0));
}
