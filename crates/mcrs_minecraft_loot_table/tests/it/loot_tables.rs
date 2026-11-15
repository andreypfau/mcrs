use std::sync::LazyLock;

use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::{load_registries, test_additions, test_packs};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{ContextParam, LootContext, ParamNotAllowed};
use mcrs_minecraft_loot_table::{
    LootIssue, LootProblem, LootScratch, LootTableFile, LootTables, RollError,
};
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_test_alloc::allocations;

use crate::fixture::{LOOT, block_table, blocks, context, names, state, stream, tool};

const TABLES: [(&str, &str); 7] = [
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
];

static ADDED: LazyLock<(Registries, LootTables, Vec<LootIssue>)> = LazyLock::new(|| {
    let mut additions = test_additions();
    for (name, table) in TABLES {
        let (namespace, path) = name.split_once(':').expect("a table name");
        additions.files.insert(
            "t",
            format!("data/{namespace}/loot_table/{path}.json"),
            table,
        );
    }
    additions.embedded_packs.push("t".to_owned());
    let (registries, _) = load_registries(&test_packs(), additions).expect("the pack loads");
    let (loot, issues) = LootTables::bake(blocks(), &registries, &test_item_table());
    (registries, loot, issues)
});

fn added_table(name: &str) -> Id<mcrs_minecraft_loot::loot::LootTable> {
    let name = ResourceLocation::read(name).expect("a table name");
    ADDED
        .0
        .registry::<LootTableFile>()
        .and_then(|tables| tables.by_name(name))
        .expect("an added table")
}

/// LOOT-CTX-013: a part that reads a parameter outside the type of its table is an issue at its
/// path.
#[test]
fn loot_ctx_013_a_part_that_reads_outside_the_table_type_is_an_issue() {
    let issues: Vec<_> = ADDED
        .2
        .iter()
        .filter(|issue| issue.table.as_str().starts_with("t:"))
        .map(|issue| (issue.table.as_str(), issue.path.as_str(), &issue.problem))
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

/// LOOT-CTX-012: a context without a parameter that its key set requires, a context with a
/// parameter that its key set does not allow, and an id that names no table, roll nothing.
#[test]
fn loot_ctx_012_a_context_without_a_required_parameter_is_refused() {
    let stone = state("minecraft:stone", &[]);
    let table = block_table(&LOOT.0, stone);
    let mut without_tool = context(stone, None);
    without_tool.tool = None;
    let mut empty_with_origin =
        LootContext::try_new(ContextKeySet::Empty).expect("a source key set");
    empty_with_origin.origin = context(stone, None).origin;
    let unknown = Id::checked(u16::MAX, usize::from(u16::MAX) + 1).expect("an id");
    let mut stacks = Vec::new();
    for (table, context, error) in [
        (
            table,
            without_tool,
            RollError::MissingParam(ContextParam::Tool),
        ),
        (
            table,
            empty_with_origin,
            RollError::UnexpectedParam(ContextParam::Origin),
        ),
        (
            unknown,
            context(stone, None),
            RollError::UnknownTable(unknown),
        ),
    ] {
        let rolled = LOOT.0.roll(
            table,
            &context,
            &NoWorld,
            &mut stream(&LOOT.0, block_table(&LOOT.0, stone), 0),
            &mut LootScratch::new(),
            &mut stacks,
        );
        assert_eq!(rolled, Err(error));
    }
    assert_eq!(stacks, []);
}

fn roll_added(name: &str) -> Vec<(String, u8)> {
    roll_with_stream(name).0
}

fn roll_with_stream(name: &str) -> (Vec<(String, u8)>, XoroshiroRandom) {
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
    (names(stacks), random)
}

/// LOOT-EVL-011, LOOT-EVL-012: each stack goes through the modifier of its composite entry and
/// then the table modifier, with their draws, before the next stack. LOOT-EVL-002: a pool takes
/// the draw of `bonus_rolls` also with the luck 0. Each binomial with `p` 1 and each float uniform
/// takes one float draw per trial.
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

/// LOOT-CTX-012: a table without `type`, of the type `generic`, rolls from a context of the key
/// set `block`.
#[test]
fn loot_ctx_012_a_table_without_type_rolls_from_a_block_context() {
    assert_eq!(roll_added("t:untyped"), [("minecraft:stick".to_owned(), 1)]);
}

/// LOOT-EVL-001, LOOT-OUT-001: a table of another type rolls by its id from a block context, and
/// the division makes stacks of the maximum stack size 16 and the rest.
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

/// LOOT-OUT-001: a count of 0 and a negative count become the empty stack, which stays in the
/// drop list; a count of 300 becomes stacks of 64 and the rest.
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

/// LOOT-RND-001: the keyed stream of a table depends on its name, not on its id: a load with one
/// more table before it gives the same draws.
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

/// A roll of a block table whose stacks have no patch allocates nothing once the buffers have
/// grown.
#[test]
fn a_roll_of_a_block_table_allocates_nothing_after_warm_up() {
    let fortune = tool("minecraft:diamond_pickaxe", &[("minecraft:fortune", 3)]);
    let cases = [
        state("minecraft:stone", &[]),
        state("minecraft:coal_ore", &[]),
        state("minecraft:wheat", &[("age", "7")]),
    ];
    let mut scratch = LootScratch::new();
    let mut stacks = Vec::new();
    let roll = |state, stacks: &mut Vec<_>, scratch: &mut LootScratch, tick| {
        let table = block_table(&LOOT.0, state);
        let mut random = stream(&LOOT.0, table, tick);
        let context: LootContext<'_, ()> = context(state, Some(&fortune));
        let before = allocations();
        LOOT.0
            .roll(table, &context, &NoWorld, &mut random, scratch, stacks)
            .expect("a block context");
        allocations() - before
    };
    for state in cases {
        roll(state, &mut stacks, &mut scratch, 0);
    }
    stacks.clear();
    let allocated: usize = cases
        .into_iter()
        .map(|state| roll(state, &mut stacks, &mut scratch, 1))
        .sum();
    assert_eq!((allocated, stacks.len() >= cases.len()), (0, true));
}
