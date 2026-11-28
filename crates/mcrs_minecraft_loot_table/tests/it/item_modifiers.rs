use std::borrow::Cow;
use std::iter;
use std::sync::LazyLock;

use glam::DVec3;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_game_registries::registry_load::{
    load_registries, test_additions, test_packs, test_registries,
};
use mcrs_minecraft_item_stack::{
    AttributeId, AttributeModifier, AttributeModifiers, Count, Damage, ItemStack, SlotGroup,
};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::{
    ContextParam, LootContext, LootWorld, ParamNotAllowed, between, bounded,
};
use mcrs_minecraft_loot_table::{
    IssueOwner, LootIssue, LootProblem, LootScratch, LootTableFile, LootTables,
};
use mcrs_minecraft_predicate::{BlockHost, DamageSource};
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation, ResourceLocationBuf};

use crate::fixture::{Around, blocks, context, item, state, stream, tool};
use crate::loot_tables::{ADDED, added_table};

fn stack(name: &str, count: u8) -> ItemStack {
    ItemStack::new(item(name), Count::new(count).expect("a count above 0"))
}

#[test]
fn loot_mod_count_damage_enchant_modifiers_give_stacks_without_draws() {
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
                &BlockHost::none(),
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

fn named_files() -> [(&'static str, String); 15] {
    [
        (
            "item_modifier/three",
            r#"{"type": "minecraft:set_count", "count": 3}"#.to_owned(),
        ),
        (
            "item_modifier/plus_one",
            r#"{"type": "minecraft:set_count", "count": 1, "add": true}"#.to_owned(),
        ),
        (
            "item_modifier/via",
            r#"["n:three", "n:plus_one"]"#.to_owned(),
        ),
        ("item_modifier/loop_a", r#""n:loop_b""#.to_owned()),
        ("item_modifier/loop_b", r#"["n:loop_a"]"#.to_owned()),
        ("item_modifier/bad", r#"["n:missing"]"#.to_owned()),
        (
            "item_modifier/refill",
            r#"{"type": "minecraft:modify_contents", "component": "minecraft:container",
             "modifier": "n:nowhere"}"#
                .to_owned(),
        ),
        (
            "item_modifier/fill",
            r#"{"type": "minecraft:set_contents", "component": "minecraft:container",
             "entries": [{"type": "minecraft:loot_table", "value": "n:filled"}]}"#
                .to_owned(),
        ),
        ("loot_table/filled", stick_with(r#""n:fill""#)),
        (
            "item_modifier/looting",
            r#"{"type": "minecraft:enchanted_count_increase", "enchantment": "minecraft:looting",
             "count": 1}"#
                .to_owned(),
        ),
        (
            "loot_table/block_looting",
            r#"{"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item",
             "name": "minecraft:stick", "modifier": "n:looting"}]}]}"#
                .to_owned(),
        ),
        (
            "tags/item_modifier/both",
            r#"{"values": ["n:three", "n:plus_one"]}"#.to_owned(),
        ),
        ("loot_table/keyed", stick_with(r#""n:via""#)),
        ("loot_table/tagged", stick_with(r##""#n:both""##)),
        (
            "loot_table/in_sequence",
            stick_with(r#"{"type": "minecraft:sequence", "functions": "n:three"}"#),
        ),
    ]
}

fn stick_with(modifier: &str) -> String {
    format!(
        r#"{{"pools": [{{"rolls": 1, "entries": [{{"type": "minecraft:item",
         "name": "minecraft:stick", "modifier": {modifier}}}]}}]}}"#
    )
}

fn modifier_chain(chain: &str, modifiers: usize) -> Vec<(String, String)> {
    let table = format!(
        r#"{{"pools": [{{"rolls": 1, "modifier": "n:{chain}/0",
         "entries": [{{"type": "minecraft:item", "name": "minecraft:stick"}}]}}]}}"#
    );
    let links = (0..modifiers).map(|link| {
        let text = match link + 1 == modifiers {
            true => r#"{"type": "minecraft:set_count", "count": 5}"#.to_owned(),
            false => format!(r#""n:{chain}/{}""#, link + 1),
        };
        (format!("data/n/item_modifier/{chain}/{link}.json"), text)
    });
    iter::once((format!("data/n/loot_table/{chain}.json"), table))
        .chain(links)
        .collect()
}

static NAMED: LazyLock<(Registries, LootTables, Vec<LootIssue>)> = LazyLock::new(|| {
    let mut additions = test_additions();
    let tables = [
        ("filtered", FILTERED.to_owned()),
        ("looting", looting(CONSTANT_HALF, 0)),
        ("looting_limit", looting(CONSTANT_HALF, 2)),
        ("looting_uniform", looting(UNIFORM, 0)),
        ("attributes", ATTRIBUTES.to_owned()),
    ];
    let files = named_files()
        .into_iter()
        .map(|(path, text)| (format!("data/n/{path}.json"), text))
        .chain(tables.map(|(name, text)| (format!("data/n/loot_table/{name}.json"), text)))
        .chain(modifier_chain("fit", 63))
        .chain(modifier_chain("over", 64));
    for (path, text) in files {
        additions.files.insert("n", path, text);
    }
    additions.embedded_packs.push("n".to_owned());
    let (registries, _) = load_registries(&test_packs(), additions).expect("the pack loads");
    let (loot, issues) = LootTables::bake(blocks(), &registries, &test_item_table());
    (registries, loot, issues)
});

fn named(name: &str) -> Id<LootTable> {
    let name = ResourceLocation::read(name).expect("a table name");
    NAMED
        .0
        .registry::<LootTableFile>()
        .and_then(|tables| tables.by_name(name))
        .expect("an added table")
}

fn roll_named<W: LootWorld<()>>(
    name: &str,
    context: &LootContext<'_, ()>,
    world: &W,
) -> (Vec<Option<ItemStack>>, XoroshiroRandom) {
    let table = named(name);
    let mut random = stream(&NAMED.1, table, 0);
    let mut stacks = Vec::new();
    NAMED
        .1
        .roll(
            table,
            context,
            world,
            &mut random,
            &mut LootScratch::new(),
            &mut stacks,
        )
        .expect("a valid context");
    (stacks, random)
}

#[test]
fn loot_mod_010_named_item_modifiers_resolve_through_the_reference_walk() {
    let mut issues: Vec<_> = NAMED
        .2
        .iter()
        .filter(|issue| issue.name.namespace() == "n")
        .map(|issue| {
            let place = (issue.name.as_str(), issue.owner, issue.path.as_str());
            (place, issue.problem.clone())
        })
        .collect();
    issues.sort_by(|left, right| left.0.0.cmp(right.0.0));
    let name = |name: &str| ResourceLocationBuf::read(name).expect("a name");
    assert_eq!(
        issues,
        [
            (
                ("n:bad", IssueOwner::ItemModifier, "[0]"),
                LootProblem::UnknownModifier("n:missing".to_owned())
            ),
            (
                (
                    "n:block_looting",
                    IssueOwner::Table,
                    "pools[0].entries[0].modifier"
                ),
                LootProblem::ParamNotAllowed(ParamNotAllowed {
                    param: ContextParam::AttackingEntity,
                    key_set: ContextKeySet::Block,
                })
            ),
            (
                ("n:fill", IssueOwner::ItemModifier, ".entries[0].value"),
                LootProblem::Cycle(vec![name("n:filled"), name("n:fill")])
            ),
            (
                ("n:loop_b", IssueOwner::ItemModifier, "[0]"),
                LootProblem::Cycle(vec![name("n:loop_a"), name("n:loop_b")])
            ),
            (
                ("n:over/62", IssueOwner::ItemModifier, ""),
                LootProblem::TooDeep
            ),
            (
                ("n:refill", IssueOwner::ItemModifier, ".modifier"),
                LootProblem::UnknownModifier("n:nowhere".to_owned())
            ),
        ]
    );
    let context = context(state("minecraft:stone", &[]), None);
    for (table, count) in [
        ("n:keyed", 4),
        ("n:tagged", 4),
        ("n:in_sequence", 3),
        ("n:fit", 5),
        ("n:over", 1),
    ] {
        assert_eq!(
            roll_named(table, &context, &BlockHost::none()),
            (
                vec![Some(stack("minecraft:stick", count))],
                stream(&NAMED.1, named(table), 0)
            ),
            "{table}"
        );
    }
}

const FILTERED: &str = r#"{"pools": [
    {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]},
    {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:snowball"}]}],
    "modifier": [{"type": "minecraft:filtered", "item_filter": {"items": "minecraft:stick"},
        "on_pass": {"type": "minecraft:set_count",
            "count": {"type": "minecraft:uniform", "min": 1, "max": 3}},
        "on_fail": {"type": "minecraft:discard"}}]}"#;

#[test]
fn loot_mod_052_filtered_applies_the_branch_of_the_test() {
    let table = named("n:filtered");
    let mut expected = stream(&NAMED.1, table, 0);
    let count = between(&mut expected, 1, 3).expect("a uniform count");
    let count = u8::try_from(count).expect("a small count");
    assert_eq!(
        roll_named(
            "n:filtered",
            &context(state("minecraft:stone", &[]), None),
            &BlockHost::none()
        ),
        (vec![Some(stack("minecraft:stick", count)), None], expected)
    );
}

const CONSTANT_HALF: &str = "0.5";
const UNIFORM: &str = r#"{"type": "minecraft:uniform", "min": 0, "max": 1}"#;

fn looting(count: &str, limit: i32) -> String {
    stick_with(&format!(
        r#"{{"type": "minecraft:enchanted_count_increase", "enchantment": "minecraft:looting",
         "count": {count}, "limit": {limit}}}"#
    ))
}

fn entity_context(attacker: Option<()>) -> LootContext<'static, ()> {
    let mut context = LootContext::try_new(ContextKeySet::Entity).expect("a source key set");
    context.origin = Some(DVec3::ZERO);
    context.this_entity = Some(());
    context.damage_source = Some(DamageSource {
        damage_type: Id::checked(0, 1).expect("an id"),
        direct_entity: None,
        causing_entity: None,
    });
    context.attacking_entity = attacker;
    context
}

#[test]
fn loot_mod_013_enchanted_count_increase_reads_the_attacker_equipment() {
    let looting = tool("minecraft:iron_sword", &[("minecraft:looting", 3)]);
    let plain = tool("minecraft:iron_sword", &[]);
    let rows: [(&str, Option<()>, Option<&ItemStack>, u8); 5] = [
        ("n:looting", Some(()), Some(&looting), 3),
        ("n:looting_limit", Some(()), Some(&looting), 2),
        ("n:looting_uniform", None, Some(&looting), 1),
        ("n:looting_uniform", Some(()), None, 1),
        ("n:looting_uniform", Some(()), Some(&plain), 1),
    ];
    for (table, attacker, weapon, count) in rows {
        let context = entity_context(attacker);
        let world = Around {
            blocks: &[],
            main_hand: weapon,
        };
        let rolled = roll_named(table, &context, &world);
        assert_eq!(
            rolled,
            (
                vec![Some(stack("minecraft:stick", count))],
                stream(&NAMED.1, named(table), 0)
            ),
            "{table} {attacker:?} {weapon:?}"
        );
    }
}

const ATTRIBUTES: &str = r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item",
    "name": "minecraft:iron_sword", "modifier": [{"type": "minecraft:set_attributes", "replace": false,
    "modifiers": [
        {"attribute": "minecraft:armor", "id": "n:armor", "amount": 2,
         "operation": "add_value", "slot": ["head", "chest"]},
        {"attribute": "minecraft:attack_damage", "id": "minecraft:base_attack_damage",
         "amount": 9, "operation": "add_multiplied_total", "slot": "mainhand"}]}]}]}]}"#;

#[test]
fn loot_mod_026_set_attributes_draws_the_slot_of_each_element() {
    let table = named("n:attributes");
    let mut expected = stream(&NAMED.1, table, 0);
    let armor_slot = bounded(&mut expected, 2).expect("a slot index");
    bounded(&mut expected, 1).expect("a slot index");
    let armor_slot =
        [SlotGroup::Head, SlotGroup::Chest][usize::try_from(armor_slot).expect("an index")];
    let attribute = |name: &str| {
        let id = test_registries()
            .entries::<Attribute>()
            .and_then(|attributes| attributes.by_name(ResourceLocation::read(name).ok()?))
            .expect("a vanilla attribute");
        AttributeId(u16::try_from(id.index()).expect("a small id"))
    };
    let sword = item("minecraft:iron_sword");
    let mut modifiers: Vec<AttributeModifier> = test_item_table()
        .prototype(sword)
        .and_then(|prototype| prototype.get::<AttributeModifiers>())
        .expect("the attribute modifiers of a sword")
        .modifiers
        .iter()
        .filter(|modifier| modifier.id().path() != "base_attack_damage")
        .cloned()
        .collect();
    let name = |name: &str| ResourceLocationBuf::read(name).expect("a name");
    let mut armor = AttributeModifier::new(
        attribute("minecraft:armor"),
        name("n:armor"),
        2.0,
        ModifierOperation::AddValue,
    );
    armor.slot = armor_slot;
    let mut damage = AttributeModifier::new(
        attribute("minecraft:attack_damage"),
        name("minecraft:base_attack_damage"),
        9.0,
        ModifierOperation::AddMultipliedTotal,
    );
    damage.slot = SlotGroup::Mainhand;
    modifiers.extend([armor, damage]);
    let modifiers = AttributeModifiers {
        modifiers: Cow::Owned(modifiers),
    };
    assert_eq!(
        roll_named(
            "n:attributes",
            &context(state("minecraft:stone", &[]), None),
            &BlockHost::none()
        ),
        (
            vec![Some(stack("minecraft:iron_sword", 1).set(modifiers))],
            expected
        )
    );
}
