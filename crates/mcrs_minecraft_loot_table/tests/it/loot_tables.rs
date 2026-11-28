use std::sync::LazyLock;

use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::{load_registries, test_additions, test_packs};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{ContextParam, LootContext, ParamNotAllowed};
use mcrs_minecraft_loot_table::{
    LootIssue, LootProblem, LootScratch, LootTableFile, LootTables, RollError,
};
use mcrs_minecraft_predicate::BlockHost;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_test_alloc::allocations;

use crate::fixture::{LOOT, block_table, blocks, context, names, state, stream, tool};

const TABLES: &[(&str, &str)] = &[
    ("t:blocks/a_first", r#"{"pools": []}"#),
    (
        "t:untyped",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}]}"#,
    ),
    (
        "t:counts",
        r#"{"pools": [
            {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick",
             "modifier": {"type": "minecraft:set_count", "count": 0}}]},
            {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick",
             "modifier": {"type": "minecraft:set_count", "count": -3}}]},
            {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick",
             "modifier": {"type": "minecraft:set_count", "count": 300}}]}]}"#,
    ),
    (
        "t:reads_tool",
        r#"{"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            {"type": "minecraft:item", "name": "minecraft:stick"},
            {"type": "minecraft:item", "name": "minecraft:stick",
             "condition": {"type": "minecraft:match_tool"}}]}]}"#,
    ),
    (
        "t:gift",
        r#"{"type": "minecraft:gift", "pools": [
            {"rolls": 2, "entries": [{"type": "minecraft:item", "name": "minecraft:stick",
             "modifier": {"type": "minecraft:set_count", "count": 3}}]},
            {"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:snowball",
             "modifier": {"type": "minecraft:set_count", "count": 70}}]}]}"#,
    ),
    (
        "t:modifier_order",
        r#"{"pools": [{"rolls": 2, "entries": [{"type": "minecraft:group",
            "children": [{"type": "minecraft:item", "name": "minecraft:stick"}],
            "modifier": {"type": "minecraft:set_count",
             "count": {"type": "minecraft:binomial", "n": 2, "p": 1.0}}}]}],
         "modifier": {"type": "minecraft:set_count", "add": true,
          "count": {"type": "minecraft:binomial", "n": 3, "p": 1.0}}}"#,
    ),
    (
        "t:bonus_rolls",
        r#"{"pools": [{"rolls": 1,
            "bonus_rolls": {"type": "minecraft:uniform", "min": 0, "max": 2},
            "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}]}"#,
    ),
    (
        "t:tag_all",
        r##"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:tag", "items": "#t:pair"}]}]}"##,
    ),
    (
        "t:tag_each",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:tag", "expand": true,
            "items": ["minecraft:stick", "minecraft:snowball"]}]}]}"#,
    ),
    (
        "t:nested_in_order",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": [
            "t:modifier_order",
            {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:snowball"}]}]}
        ]}]}]}"#,
    ),
    (
        "t:nested_undivided",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "t:gift",
            "modifier": {"type": "minecraft:set_count", "count": -60, "add": true}}]}]}"#,
    ),
    (
        "t:nested_each",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "expand": true, "value": [
            "t:untyped",
            {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:snowball"}]}]}
        ]}]}]}"#,
    ),
    (
        "t:dynamic",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:dynamic", "name": "minecraft:contents"}]}]}"#,
    ),
    (
        "t:dynamic_filtered",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:dynamic", "name": "minecraft:contents",
            "modifier": [{"type": "minecraft:set_count", "count": 2},
                {"type": "minecraft:filtered", "item_filter": {"count": 2},
                 "on_pass": {"type": "minecraft:set_count", "count": 3}}]}]}]}"#,
    ),
    (
        "t:dynamic_unknown",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:dynamic", "name": "t:contents"}]}]}"#,
    ),
    (
        "t:quality",
        r#"{"pools": [{"rolls": 1, "entries": [
            {"type": "minecraft:item", "name": "minecraft:stick"},
            {"type": "minecraft:item", "name": "minecraft:snowball", "weight": 3, "quality": -1}]}]}"#,
    ),
    (
        "t:luck_rolls",
        r#"{"pools": [{"rolls": 1, "bonus_rolls": 1.5,
            "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}]}"#,
    ),
    (
        "f:a",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "f:b"}]}]}"#,
    ),
    (
        "f:b",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value":
            {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "f:a"}]}]}}]}]}"#,
    ),
    (
        "f:self",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:group", "children": [
            {"type": "minecraft:item", "name": "minecraft:stick"},
            {"type": "minecraft:loot_table", "value": "f:self"}]}]}]}"#,
    ),
    (
        "f:twice",
        r#"{"pools": [
            {"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "t:untyped"}]},
            {"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "t:untyped"}]}]}"#,
    ),
    (
        "f:no_children",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:sequence"}]}]}"#,
    ),
    (
        "f:alternatives",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:alternatives", "children": [
            {"type": "minecraft:item", "name": "minecraft:stick"},
            {"type": "minecraft:item", "name": "minecraft:snowball"}]}]}]}"#,
    ),
    (
        "f:chest_nests",
        r#"{"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            {"type": "minecraft:loot_table", "value": "f:reads_tool"}]}]}"#,
    ),
    (
        "f:reads_tool",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick",
            "condition": {"type": "minecraft:match_tool"}}]}]}"#,
    ),
    (
        "f:unknown_table",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:loot_table", "value": "f:missing"}]}]}"#,
    ),
    (
        "m:limit_min_above_max",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}],
         "modifier": [{"type": "minecraft:set_count", "count": 1},
          {"type": "minecraft:limit_count", "limit": {"min": 3, "max": 1}}]}"#,
    ),
    (
        "m:empty_then_add",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": [{"type": "minecraft:set_count", "count": -1},
          {"type": "minecraft:set_damage", "damage": 0.5},
          {"type": "minecraft:set_item", "item": "minecraft:stick"},
          {"type": "minecraft:explosion_decay"},
          {"type": "minecraft:set_count", "count": 2, "add": true}]}"#,
    ),
    (
        "m:failed_condition",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}],
         "modifier": {"type": "minecraft:sequence",
          "condition": {"type": "minecraft:inverted", "term": {"type": "minecraft:survives_explosion"}},
          "functions": [{"type": "minecraft:set_count", "count": {"type": "minecraft:uniform", "min": 2, "max": 9}}]}}"#,
    ),
    (
        "m:discard",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}],
         "modifier": [{"type": "minecraft:set_count", "count": 3}, {"type": "minecraft:discard"},
          {"type": "minecraft:set_count", "count": 2}]}"#,
    ),
    (
        "m:set_item",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": [{"type": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 2}},
          {"type": "minecraft:set_count", "count": 3},
          {"type": "minecraft:set_item", "item": "minecraft:stick"}]}"#,
    ),
    (
        "m:damage_add",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": [{"type": "minecraft:set_damage", "damage": 0.5},
          {"type": "minecraft:set_damage", "damage": 0.25, "add": true}]}"#,
    ),
    (
        "m:enchantments",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": [{"type": "minecraft:set_enchantments",
           "enchantments": {"minecraft:sharpness": 3, "minecraft:unbreaking": 2}},
          {"type": "minecraft:set_enchantments", "add": true,
           "enchantments": {"minecraft:sharpness": 2, "minecraft:unbreaking": -5}}]}"#,
    ),
    (
        "m:no_compatible_enchantment",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}],
         "modifier": {"type": "minecraft:enchant_randomly", "options": "minecraft:sharpness"}}"#,
    ),
    (
        "m:full_durability",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": [{"type": "minecraft:set_damage", "damage": 0.5},
          {"type": "minecraft:set_damage", "damage": 1}]}"#,
    ),
    (
        "m:enchantment_level_zero",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:iron_sword"}]}],
         "modifier": {"type": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 0}}}"#,
    ),
    (
        "f:set_item_air",
        r#"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]}],
         "modifier": {"type": "minecraft:set_item", "item": "minecraft:air"}}"#,
    ),
    (
        "f:unknown_tag",
        r##"{"pools": [{"rolls": 1, "entries": [{"type": "minecraft:tag", "items": "#f:missing"}]}]}"##,
    ),
];

pub const CHAIN: usize = 66;

fn chain_table(link: usize) -> String {
    let entry = if link + 1 == CHAIN {
        r#"{"type": "minecraft:item", "name": "minecraft:stick"}"#.to_owned()
    } else {
        let next = link + 1;
        format!(r#"{{"type": "minecraft:loot_table", "value": "d:chain/{next}"}}"#)
    };
    format!(r#"{{"pools": [{{"rolls": 1, "entries": [{entry}]}}]}}"#)
}

fn table_chain(chain: &str, tables: usize, rolls: &str) -> Vec<(String, String)> {
    (0..tables)
        .map(|link| {
            let next = link + 1;
            let (rolls, entry) = match next == tables {
                true => (
                    rolls.to_owned(),
                    r#"{"type": "minecraft:item", "name": "minecraft:stick"}"#.to_owned(),
                ),
                false => (
                    "1".to_owned(),
                    format!(r#"{{"type": "minecraft:loot_table", "value": "c:{chain}/{next}"}}"#),
                ),
            };
            (
                format!("data/c/loot_table/{chain}/{link}.json"),
                format!(r#"{{"pools": [{{"rolls": {rolls}, "entries": [{entry}]}}]}}"#),
            )
        })
        .collect()
}

fn provider_chain(chain: &str, providers: usize) -> Vec<(String, String)> {
    let mut files = table_chain(chain, 32, &format!(r#""c:p{chain}/0""#));
    for link in 0..providers {
        let path = format!("data/c/context_int_provider/p{chain}/{link}.json");
        let next = link + 1;
        let provider = match next == providers {
            true => "1".to_owned(),
            false => format!(r#"{{"type": "minecraft:abs", "input": "c:p{chain}/{next}"}}"#),
        };
        files.push((path, provider));
    }
    files
}

struct ChainRow {
    chain: &'static str,
    tables: usize,
    rolls: &'static str,
    too_long: bool,
}

const CONSTANT: &str = r#"{"type": "minecraft:constant", "value": 1}"#;

const CHAIN_ROWS: &[ChainRow] = &[
    ChainRow {
        chain: "inline",
        tables: 64,
        rolls: CONSTANT,
        too_long: false,
    },
    ChainRow {
        chain: "keyed",
        tables: 64,
        rolls: r#""c:one""#,
        too_long: true,
    },
    ChainRow {
        chain: "nested_once",
        tables: 63,
        rolls: r#"{"type": "minecraft:abs", "input": {"type": "minecraft:constant", "value": 1}}"#,
        too_long: false,
    },
    ChainRow {
        chain: "nested_twice",
        tables: 63,
        rolls: r#"{"type": "minecraft:abs", "input": {"type": "minecraft:abs",
            "input": {"type": "minecraft:constant", "value": 1}}}"#,
        too_long: true,
    },
    ChainRow {
        chain: "in_condition",
        tables: 63,
        rolls: r#"{"type": "minecraft:conditional", "on_true": 1,
            "condition": {"type": "minecraft:random_chance",
                "chance": {"type": "minecraft:abs", "input": 1}}}"#,
        too_long: true,
    },
    ChainRow {
        chain: "via_predicate",
        tables: 62,
        rolls: r#""c:via""#,
        too_long: true,
    },
];

const CHAIN_FILES: &[(&str, &str)] = &[
    ("data/c/context_int_provider/one.json", "1"),
    (
        "data/c/context_int_provider/via.json",
        r#"{"type": "minecraft:conditional", "condition": "c:pred", "on_true": 1}"#,
    ),
    (
        "data/c/predicate/pred.json",
        r#"{"type": "minecraft:random_chance", "chance": "c:half"}"#,
    ),
    ("data/c/context_float_provider/half.json", "0.5"),
];

pub static ADDED: LazyLock<(Registries, LootTables, Vec<LootIssue>)> = LazyLock::new(|| {
    let mut additions = test_additions();
    additions.files.insert(
        "t",
        "data/t/tags/item/pair.json".to_owned(),
        r#"{"values": ["minecraft:snowball", "minecraft:stick"]}"#,
    );
    for &(name, table) in TABLES {
        let (namespace, path) = name.split_once(':').expect("a table name");
        additions.files.insert(
            "t",
            format!("data/{namespace}/loot_table/{path}.json"),
            table,
        );
    }
    for link in 0..CHAIN {
        additions.files.insert(
            "t",
            format!("data/d/loot_table/chain/{link}.json"),
            chain_table(link),
        );
    }
    for (path, text) in provider_chain("a", 32)
        .into_iter()
        .chain(provider_chain("b", 33))
    {
        additions.files.insert("t", path, text);
    }
    for row in CHAIN_ROWS {
        for (path, text) in table_chain(row.chain, row.tables, row.rolls) {
            additions.files.insert("t", path, text);
        }
    }
    for &(path, text) in CHAIN_FILES {
        additions.files.insert("t", path.to_owned(), text);
    }
    additions.embedded_packs.push("t".to_owned());
    let (registries, _) = load_registries(&test_packs(), additions).expect("the pack loads");
    let (loot, issues) = LootTables::bake(blocks(), &registries, &test_item_table());
    (registries, loot, issues)
});

pub fn added_table(name: &str) -> Id<mcrs_minecraft_loot::loot::LootTable> {
    let name = ResourceLocation::read(name).expect("a table name");
    ADDED
        .0
        .registry::<LootTableFile>()
        .and_then(|tables| tables.by_name(name))
        .expect("an added table")
}

#[test]
fn loot_ctx_013_a_part_that_reads_outside_the_table_type_is_an_issue() {
    let issues: Vec<_> = ADDED
        .2
        .iter()
        .filter(|issue| issue.name.as_str().starts_with("t:"))
        .map(|issue| (issue.name.as_str(), issue.path.as_str(), &issue.problem))
        .collect();
    let expected = LootProblem::ParamNotAllowed(ParamNotAllowed {
        param: ContextParam::Tool,
        key_set: ContextKeySet::Chest,
    });
    assert_eq!(
        issues,
        [("t:reads_tool", "pools[0].entries[1].condition", &expected)]
    );
}

#[test]
fn loot_ctx_017_a_table_chain_continues_into_its_providers() {
    let mut issues: Vec<_> = ADDED
        .2
        .iter()
        .filter(|issue| issue.name.as_str().starts_with("c:"))
        .map(|issue| (issue.name.to_string(), issue.path.as_str(), &issue.problem))
        .collect();
    issues.sort_by(|left, right| left.0.cmp(&right.0));
    let too_long = CHAIN_ROWS
        .iter()
        .filter(|row| row.too_long)
        .map(|row| format!("c:{}/{}", row.chain, row.tables - 1));
    let mut expected: Vec<_> = ["c:b/31".to_owned()]
        .into_iter()
        .chain(too_long)
        .map(|table| (table, "pools[0].rolls", &LootProblem::ChainTooLong))
        .collect();
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(issues, expected);
}

#[test]
fn loot_ctx_012_a_context_without_a_required_parameter_is_refused() {
    let stone = state("minecraft:stone", &[]);
    let table = block_table(&LOOT.0, stone);
    let mut without_tool = context(stone, None);
    without_tool.tool = None;
    let mut empty_with_origin =
        LootContext::try_new(ContextKeySet::Empty).expect("a source key set");
    empty_with_origin.origin = context(stone, None).origin;
    let mut attacked_block = context(stone, None);
    attacked_block.attacking_entity = Some(());
    let sword = tool("minecraft:iron_sword", &[]);
    let mut enchanted_item =
        LootContext::try_new(ContextKeySet::EnchantedItem).expect("a source key set");
    enchanted_item.tool = context(stone, Some(&sword)).tool;
    enchanted_item.enchantment_level = Some(1);
    let unknown = Id::checked(u16::MAX, usize::from(u16::MAX) + 1).expect("an id");
    for (table, context, expected) in [
        (
            table,
            without_tool,
            Err(RollError::MissingParam(ContextParam::Tool)),
        ),
        (
            table,
            empty_with_origin,
            Err(RollError::UnexpectedParam(ContextParam::Origin)),
        ),
        (
            table,
            attacked_block,
            Err(RollError::UnexpectedParam(ContextParam::AttackingEntity)),
        ),
        (table, enchanted_item, Ok(())),
        (
            unknown,
            context(stone, None),
            Err(RollError::UnknownTable(unknown)),
        ),
    ] {
        let mut stacks = Vec::new();
        let rolled = LOOT.0.roll(
            table,
            &context,
            &BlockHost::none(),
            &mut stream(&LOOT.0, block_table(&LOOT.0, stone), 0),
            &mut LootScratch::new(),
            &mut stacks,
        );
        assert_eq!(rolled, expected);
        if expected.is_err() {
            assert_eq!(stacks, []);
        }
    }
}

fn roll_added(name: &str) -> Vec<(String, u8)> {
    roll_with_stream(name).0
}

pub fn roll_with_stream(name: &str) -> (Vec<(String, u8)>, XoroshiroRandom) {
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
    (names(stacks), random)
}

#[test]
fn loot_evl_011_modifiers_and_bonus_rolls_take_their_draws_per_stack() {
    let stick = |count| ("minecraft:stick".to_owned(), count);
    for (name, stacks, draws) in [
        ("t:modifier_order", vec![stick(5), stick(5)], 10),
        ("t:bonus_rolls", vec![stick(1)], 1),
    ] {
        let mut expected = stream(&ADDED.1, added_table(name), 0);
        for _ in 0..draws {
            expected.next_f32();
        }
        assert_eq!(roll_with_stream(name), (stacks, expected), "{name}");
    }
}

#[test]
fn loot_ctx_012_a_table_without_type_rolls_from_a_block_context() {
    assert_eq!(roll_added("t:untyped"), [("minecraft:stick".to_owned(), 1)]);
}

#[test]
fn loot_evl_001_a_table_of_another_type_rolls_by_its_id() {
    let stick = ("minecraft:stick".to_owned(), 3);
    let snowball = |count| ("minecraft:snowball".to_owned(), count);
    assert_eq!(
        roll_added("t:gift"),
        [
            stick.clone(),
            stick,
            snowball(16),
            snowball(16),
            snowball(16),
            snowball(16),
            snowball(6)
        ]
    );
}

#[test]
fn loot_out_001_the_division_keeps_empty_stacks_and_splits_large_counts() {
    let stick = |count| ("minecraft:stick".to_owned(), count);
    let empty = (String::new(), 0);
    assert_eq!(
        roll_added("t:counts"),
        [
            empty.clone(),
            empty,
            stick(64),
            stick(64),
            stick(64),
            stick(64),
            stick(44)
        ]
    );
}

#[test]
fn loot_rnd_001_a_table_added_before_keeps_the_keyed_stream() {
    let stone = state("minecraft:stone", &[]);
    let base = block_table(&LOOT.0, stone);
    let added = block_table(&ADDED.1, stone);
    assert_ne!(
        base, added,
        "the table t:blocks/a_first comes before minecraft:blocks/stone"
    );
    let draws = |loot: &LootTables, table| {
        let mut random = stream(loot, table, 0);
        [random.next_i64(), random.next_i64()]
    };
    assert_eq!(draws(&LOOT.0, base), draws(&ADDED.1, added));
}

#[test]
fn a_roll_of_a_block_table_allocates_nothing_after_warm_up() {
    let fortune = tool("minecraft:diamond_pickaxe", &[("minecraft:fortune", 3)]);
    let stone = state("minecraft:stone", &[]);
    let contents = [Some(tool("minecraft:stick", &[]))];
    let cases = [
        (&LOOT.0, block_table(&LOOT.0, stone)),
        (
            &LOOT.0,
            block_table(&LOOT.0, state("minecraft:coal_ore", &[])),
        ),
        (
            &LOOT.0,
            block_table(&LOOT.0, state("minecraft:wheat", &[("age", "7")])),
        ),
        (&ADDED.1, added_table("t:dynamic_filtered")),
    ];
    let mut scratch = LootScratch::new();
    let mut stacks = Vec::new();
    let roll = |(loot, table): (&LootTables, _), stacks: &mut Vec<_>, scratch: &mut _, tick| {
        let mut random = stream(loot, table, tick);
        let mut context: LootContext<'_, ()> = context(stone, Some(&fortune));
        context.contents = &contents;
        let before = allocations();
        loot.roll(
            table,
            &context,
            &BlockHost::none(),
            &mut random,
            scratch,
            stacks,
        )
        .expect("a block context");
        allocations() - before
    };
    for case in cases {
        roll(case, &mut stacks, &mut scratch, 0);
    }
    stacks.clear();
    let allocated: Vec<usize> = cases
        .into_iter()
        .map(|case| roll(case, &mut stacks, &mut scratch, 1))
        .collect();
    assert_eq!(allocated, [0; 4]);
    assert_eq!(names(stacks.pop()), [("minecraft:stick".to_owned(), 3)]);
}
