use mcrs_minecraft_item::Tool;
use mcrs_minecraft_keys::{Block, block};
use mcrs_minecraft_world::item::tool::{is_correct_for_drops, mining_speed};
use mcrs_minecraft_world::registries::test_registries;

use crate::common::items;

#[test]
fn a_pickaxe_rule_matches_stone_by_bit_test() {
    let set = test_registries();
    let tags = set.tags::<Block>().unwrap();
    let stone = block::STONE;
    let dirt = block::DIRT;

    let items = items();
    let pickaxe = items
        .get(items.id_of("minecraft:iron_pickaxe").unwrap())
        .unwrap();
    let tool = pickaxe.prototype.get::<Tool>().unwrap();

    let rule = tool
        .rules
        .iter()
        .find(|rule| {
            rule.blocks
                .tag()
                .is_some_and(|tag| tags.name(tag).as_str() == "minecraft:mineable/pickaxe")
        })
        .expect("the iron pickaxe has a rule for the pickaxe tag");
    assert!(rule.blocks.contains(stone, &tags));
    assert!(!rule.blocks.contains(dirt, &tags));

    assert_eq!(mining_speed(tool, stone, &tags), 6.0);
    assert_eq!(mining_speed(tool, dirt, &tags), tool.default_mining_speed);
    assert!(is_correct_for_drops(tool, stone, &tags));
    assert!(!is_correct_for_drops(tool, dirt, &tags));
}
