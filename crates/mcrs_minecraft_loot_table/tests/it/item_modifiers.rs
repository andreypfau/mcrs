use mcrs_minecraft_item_stack::{Count, Damage, ItemStack};
use mcrs_minecraft_loot_table::{LootScratch, LootTableFile};
use mcrs_minecraft_predicate::NoWorld;

use crate::fixture::{context, item, state, stream, tool};
use crate::loot_tables::{ADDED, added_table};

fn stack(name: &str, count: u8) -> ItemStack {
    ItemStack::new(item(name), Count::new(count).expect("a count above 0"))
}

/// LOOT-MOD-003, LOOT-MOD-009, LOOT-MOD-011, LOOT-MOD-014, LOOT-MOD-015, LOOT-MOD-016,
/// LOOT-MOD-019 step 2, LOOT-MOD-022, STK-PAT-006: each table gives its stacks and takes no draw;
/// a write of the value of the prototype leaves no patch entry.
#[test]
fn loot_mod_count_item_damage_and_enchantment_modifiers_give_their_stacks_without_draws() {
    let sword = || stack("minecraft:iron_sword", 1);
    let cases: [(&str, Vec<Option<ItemStack>>); 10] = [
        (
            "m:limit_min_above_max",
            vec![Some(stack("minecraft:stick", 3))],
        ),
        ("m:empty_then_add", vec![Some(sword()), Some(sword())]),
        (
            "m:failed_condition",
            vec![Some(stack("minecraft:stick", 1))],
        ),
        ("m:discard", vec![None]),
        (
            "m:set_item",
            vec![Some({
                let mut stick = tool("minecraft:iron_sword", &[("minecraft:sharpness", 2)]);
                stick.kind = item("minecraft:stick");
                stick.count = Count::new(3).expect("a count above 0");
                stick
            })],
        ),
        ("m:damage_add", vec![Some(sword().set(Damage(62)))]),
        (
            "m:enchantments",
            vec![Some(tool(
                "minecraft:iron_sword",
                &[("minecraft:sharpness", 5)],
            ))],
        ),
        (
            "m:no_compatible_enchantment",
            vec![Some(stack("minecraft:stick", 1))],
        ),
        ("m:full_durability", vec![Some(sword())]),
        ("m:enchantment_level_zero", vec![Some(sword())]),
    ];
    for (name, expected) in cases {
        let table = added_table(name);
        let mut random = stream(&ADDED.1, table, 0);
        let mut stacks = Vec::new();
        ADDED
            .1
            .roll(
                table,
                &context(state("minecraft:stone", &[]), None),
                &NoWorld,
                &mut random,
                &mut LootScratch::new(),
                &mut stacks,
            )
            .expect("a block context");
        assert_eq!(
            (stacks, random),
            (expected, stream(&ADDED.1, table, 0)),
            "{name}"
        );
    }
}

/// REG-FLD-741, REG-FLD-765, REG-FLD-782, REG-FLD-784, REG-FLD-786, REG-FLD-789, REG-FLD-754: a
/// modifier type without a rule, a formula without one of its parameters, a value outside its
/// shape and a value past a stated bound fail the decode of the table.
fn table_with(modifier: &str) -> String {
    format!(
        r#"{{"pools": [{{"rolls": 1, "entries": [{{"type": "minecraft:item", "name": "minecraft:stick"}}]}}],
         "modifier": {modifier}}}"#
    )
}

fn bonus(rest: &str) -> String {
    format!(r#"{{"type": "minecraft:apply_bonus", "enchantment": "minecraft:fortune", {rest}}}"#)
}

fn attribute(slot: &str) -> String {
    format!(
        r#"{{"type": "minecraft:set_attributes", "modifiers": [{{"attribute": "minecraft:armor",
         "id": "t:armor", "amount": 1, "operation": "add_value", "slot": {slot}}}]}}"#
    )
}

fn strict_decode_cases() -> Vec<(String, bool)> {
    vec![
        (r#"{"type": "minecraft:nothing"}"#.to_owned(), false),
        (bonus(r#""formula": "minecraft:ore_drops""#), true),
        (
            bonus(r#""formula": "minecraft:ore_drops", "parameters": {}"#),
            true,
        ),
        (
            bonus(r#""formula": "minecraft:uniform_bonus_count""#),
            false,
        ),
        (
            bonus(
                r#""formula": "minecraft:uniform_bonus_count", "parameters": {"bonusMultiplier": 1}"#,
            ),
            true,
        ),
        (
            bonus(
                r#""formula": "minecraft:binomial_with_bonus_count", "parameters": {"extra": 3}"#,
            ),
            false,
        ),
        (
            bonus(
                r#""formula": "minecraft:binomial_with_bonus_count", "parameters": {"probability": 0.5}"#,
            ),
            false,
        ),
        (
            bonus(
                r#""formula": "minecraft:binomial_with_bonus_count", "parameters": {"extra": 3, "probability": 0.5}"#,
            ),
            true,
        ),
        (
            r#"{"type": "minecraft:set_book_cover", "generation": 3}"#.to_owned(),
            true,
        ),
        (
            r#"{"type": "minecraft:set_book_cover", "generation": 4}"#.to_owned(),
            false,
        ),
        (attribute(r#""head""#), true),
        (attribute("[]"), false),
        (
            r#"{"type": "minecraft:set_custom_model_data",
             "colors": {"mode": "append", "values": [[1.0, 0.0, 0.0], 7]}}"#
                .to_owned(),
            true,
        ),
        (
            r#"{"type": "minecraft:set_fireworks",
             "explosions": {"mode": "replace_all", "values": [false]}}"#
                .to_owned(),
            false,
        ),
        (
            r#"{"type": "minecraft:set_writable_book_pages", "mode": "append", "pages": [false]}"#
                .to_owned(),
            false,
        ),
        (
            format!(
                r#"{{"type": "minecraft:set_book_cover", "title": "{}"}}"#,
                "\u{1F600}".repeat(17)
            ),
            false,
        ),
        (
            format!(
                r#"{{"type": "minecraft:set_book_cover", "title": "{}"}}"#,
                "a".repeat(32)
            ),
            true,
        ),
    ]
}

#[test]
fn reg_fld_741_a_modifier_outside_its_rule_fails_the_decode() {
    for (modifier, decodes) in strict_decode_cases() {
        assert_eq!(
            serde_json::from_str::<LootTableFile>(&table_with(&modifier)).is_ok(),
            decodes,
            "{modifier}"
        );
    }
}
