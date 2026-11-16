use mcrs_minecraft_item_stack::{Count, ItemStack, MaxStackSize};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{ContextParam, LootContext, ParamNotAllowed};
use mcrs_minecraft_loot_table::{LootProblem, LootScratch, LootTableFile};
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::fixture::{context, item, names, state, stream, tool};
use crate::loot_tables::{ADDED, CHAIN, added_table, roll_with_stream};

fn stick(count: u8) -> (String, u8) {
    ("minecraft:stick".to_owned(), count)
}

fn snowball(count: u8) -> (String, u8) {
    ("minecraft:snowball".to_owned(), count)
}

fn fresh(name: &str) -> XoroshiroRandom {
    stream(&ADDED.1, added_table(name), 0)
}

fn roll_in(name: &str, context: &LootContext<'_, ()>) -> (Vec<Option<ItemStack>>, XoroshiroRandom) {
    let mut random = fresh(name);
    let mut stacks = Vec::new();
    ADDED
        .1
        .roll(
            added_table(name),
            context,
            &NoWorld,
            &mut random,
            &mut LootScratch::new(),
            &mut stacks,
        )
        .expect("a block context");
    (stacks, random)
}

fn with_luck(luck: f32) -> LootContext<'static, ()> {
    let mut context = context(state("minecraft:stone", &[]), None);
    context.luck = luck;
    context
}

/// LOOT-EVL-009, LOOT-EVL-010, LOOT-EVL-011, LOOT-OUT-001: a `tag` entry without `expand` makes
/// each item of its tag in member order; a `loot_table` entry evaluates a named and an inline
/// table in order, with the same generator; the stacks of a nested table reach the modifier of
/// the entry before the division. None of these takes a draw of its own.
#[test]
fn loot_evl_010_a_nested_table_rolls_in_the_same_evaluation() {
    let empty = (String::new(), 0);
    for (name, stacks, float_draws) in [
        ("t:tag_all", vec![snowball(1), stick(1)], 0),
        (
            "t:nested_in_order",
            vec![stick(5), stick(5), snowball(1)],
            10,
        ),
        (
            "t:nested_undivided",
            vec![empty.clone(), empty, snowball(10)],
            0,
        ),
    ] {
        let mut expected = fresh(name);
        for _ in 0..float_draws {
            expected.next_f32();
        }
        assert_eq!(roll_with_stream(name), (stacks, expected), "{name}");
    }
}

/// LOOT-EVL-003, LOOT-EVL-009, LOOT-EVL-010: an entry with `expand` gives one candidate for each
/// member, and the roll selects one of them with one draw `B(2)`.
#[test]
fn loot_evl_009_an_expanded_entry_gives_one_candidate_per_member() {
    for (name, members) in [
        ("t:tag_each", [stick(1), snowball(1)]),
        ("t:nested_each", [stick(1), snowball(1)]),
    ] {
        let mut expected = fresh(name);
        let pick = expected.next_u32_bound(2);
        let selected = members
            .into_iter()
            .nth(usize::try_from(pick).expect("a small index"))
            .expect("a member");
        assert_eq!(roll_with_stream(name), (vec![selected], expected), "{name}");
    }
}

/// LOOT-EVL-007: a `dynamic` entry makes the stacks that the context gives for its name, in
/// order, with their component patch and with the empty stack, and not the stacks of another
/// name. A name that no source gives makes nothing. It takes no draw. LOOT-OUT-001, STK-CNT-002:
/// a stack is divided at the maximum stack size of its patch.
#[test]
fn loot_evl_007_a_dynamic_entry_makes_the_stacks_of_the_context() {
    let enchanted = tool("minecraft:diamond_pickaxe", &[("minecraft:fortune", 3)]);
    let sticks = ItemStack::new(item("minecraft:stick"), Count::new(3).expect("a count"));
    let given = [Some(enchanted), None, Some(sticks)];
    let large = ItemStack::new(item("minecraft:stick"), Count::new(99).expect("a count"));
    let large = [Some(large.set(MaxStackSize(99)))];
    for (table, contents, sherds, expected) in [
        ("t:dynamic", &given[..], &[][..], given.to_vec()),
        ("t:dynamic", &[][..], &given[..], vec![]),
        ("t:dynamic_unknown", &given[..], &given[..], vec![]),
        ("t:dynamic", &large[..], &[][..], large.to_vec()),
    ] {
        let mut context = context(state("minecraft:stone", &[]), None);
        context.contents = contents;
        context.sherds = sherds;
        assert_eq!(
            roll_in(table, &context),
            (expected, fresh(table)),
            "{table}"
        );
    }
}

/// LOOT-EVL-003, LOOT-CTX-015: the weight of a candidate is `floor(weight + quality × luck)`,
/// at least 0. A roll with one candidate of a weight above 0 takes no draw; otherwise it takes
/// `B(t)` over the sum `t`. The first candidate is a stick of weight 1; the second a snowball
/// of weight 3 and quality -1.
#[test]
fn loot_evl_003_quality_and_luck_change_the_weights() {
    for (luck, total) in [(0.0, Some(4)), (-1.0, Some(5)), (2.5, None), (3.0, None)] {
        let mut expected = fresh("t:quality");
        let stack = match total {
            Some(total) => match expected.next_u32_bound(total) {
                0 => stick(1),
                _ => snowball(1),
            },
            None => stick(1),
        };
        let (stacks, random) = roll_in("t:quality", &with_luck(luck));
        assert_eq!((names(stacks), random), (vec![stack], expected), "{luck}");
    }
}

/// LOOT-EVL-002, LOOT-CTX-015: a pool rolls `rolls + floor(bonus_rolls × luck)` times, and not
/// at all when that is 0 or less.
#[test]
fn loot_evl_002_the_luck_adds_bonus_rolls() {
    for (luck, rolls) in [(0.0, 1), (2.0, 4), (0.5, 1), (-1.0, 0)] {
        let (stacks, random) = roll_in("t:luck_rolls", &with_luck(luck));
        assert_eq!(
            (names(stacks), random),
            (vec![stick(1); rolls], fresh("t:luck_rolls")),
            "{luck}"
        );
    }
}

/// LOOT-CTX-013, LOOT-CTX-014, LOOT-CTX-016: a reference back to a table that contains it, also
/// through an inline table, a composite entry without children, an `alternatives` child other
/// than the last without a condition, and a nested table that reads a parameter outside the
/// type of the table that contains it, a table and an item tag that the load does not hold, are
/// issues at their paths. A table named two times is not a cycle. A table with a cycle still
/// rolls, and ends. Which table of a cycle reports it depends on the order of the load.
#[test]
fn loot_ctx_016_entry_faults_are_issues_with_their_path() {
    let mut issues: Vec<_> = ADDED
        .2
        .iter()
        .filter(|issue| issue.name.namespace() == "f")
        .map(|issue| {
            (
                issue.name.as_str(),
                issue.path.as_str(),
                issue.problem.clone(),
            )
        })
        .collect();
    issues.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
    let cycle_reports = [
        (
            vec!["f:a", "f:b"],
            vec![
                ("f:a", "pools[0].entries[0].value"),
                ("f:b", "pools[0].entries[0].value.pools[0].entries[0].value"),
            ],
        ),
        (
            vec!["f:self"],
            vec![("f:self", "pools[0].entries[0].children[1].value")],
        ),
    ];
    let mut cycles: Vec<Vec<&str>> = Vec::new();
    for (table, path, problem) in &issues {
        if let LootProblem::Cycle(members) = problem {
            let mut members: Vec<&str> = members.iter().map(ResourceLocationBuf::as_str).collect();
            members.sort_unstable();
            let reports = cycle_reports
                .iter()
                .find(|(expected, _)| *expected == members);
            assert!(
                reports.is_some_and(|(_, reports)| reports.contains(&(*table, *path))),
                "{table} at {path}: {members:?}"
            );
            cycles.push(members);
        }
    }
    cycles.sort_unstable();
    assert_eq!(cycles, [vec!["f:a", "f:b"], vec!["f:self"]]);
    issues.retain(|(_, _, problem)| !matches!(problem, LootProblem::Cycle(_)));
    let tool = LootProblem::ParamNotAllowed(ParamNotAllowed {
        param: ContextParam::Tool,
        key_set: ContextKeySet::Chest,
    });
    assert_eq!(
        issues,
        [
            (
                "f:alternatives",
                "pools[0].entries[0].children[0]",
                LootProblem::AlternativeWithoutCondition
            ),
            ("f:chest_nests", "pools[0].entries[0].value", tool),
            (
                "f:no_children",
                "pools[0].entries[0]",
                LootProblem::NoChildren
            ),
            ("f:set_item_air", "modifier", LootProblem::SetItemAir),
            (
                "f:unknown_table",
                "pools[0].entries[0].value",
                LootProblem::UnknownTable("f:missing".to_owned())
            ),
            (
                "f:unknown_tag",
                "pools[0].entries[0].items",
                LootProblem::UnknownTag("f:missing".to_owned())
            ),
        ]
    );
    assert_eq!(roll_with_stream("f:a"), (vec![], fresh("f:a")));
}

/// LOOT-CTX-017: in a line of 66 tables, the chain from the first table gets 65 members at the
/// entry of the table 64, and the chain from the second at the entry of the table 65. The cut
/// keeps each chain of the roll at 64 tables.
#[test]
fn loot_ctx_017_a_chain_of_nested_tables_past_the_limit_is_an_issue_at_each_place() {
    let issues: Vec<_> = ADDED
        .2
        .iter()
        .filter(|issue| issue.name.namespace() == "d")
        .map(|issue| (issue.name.as_str(), issue.path.as_str(), &issue.problem))
        .collect();
    let entry = "pools[0].entries[0].value";
    assert_eq!(
        issues,
        [
            ("d:chain/63", entry, &LootProblem::TooDeep),
            ("d:chain/64", entry, &LootProblem::TooDeep),
        ]
    );
    for (link, stacks) in [(0, vec![]), (CHAIN - 64, vec![stick(1)])] {
        let name = format!("d:chain/{link}");
        assert_eq!(roll_with_stream(&name), (stacks, fresh(&name)), "{name}");
    }
}

/// REG-FLD-790, REG-FLD-797, REG-FLD-801: an entry of a type without a rule fails the decode of
/// its table; a `slots` entry decodes with a key, a bare `group` and a typed slot source.
#[test]
fn reg_fld_790_an_unknown_entry_type_fails_the_decode() {
    let table = |entry: &str| format!(r#"{{"pools": [{{"rolls": 1, "entries": [{entry}]}}]}}"#);
    for (entry, decodes) in [
        (r#"{"type": "minecraft:nothing"}"#, false),
        (
            r#"{"type": "minecraft:slots", "slot_source": ["t:hotbar", {"type": "minecraft:empty"}]}"#,
            true,
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<LootTableFile>(&table(entry)).is_ok(),
            decodes,
            "{entry}"
        );
    }
}
