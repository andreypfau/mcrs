use std::sync::LazyLock;

use mcrs_minecraft_engine_registries::{test_corpus, test_item_table};
use mcrs_minecraft_game_registries::registry_load::{
    load_registries, test_additions, test_packs, test_registries,
};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    ConditionBaker, ConditionProblem, ContextParam, FloatProvider, FloatProviderFile, IntProvider,
    IntProviderFile, LootContext, LootInputs, MemberKind, NamedEntries, NamedIssue, bounded,
};
use mcrs_minecraft_predicate::{BlockHost, PredicateScratch};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registries::Registries;

const SEED: u64 = 42;

pub static VANILLA: LazyLock<NamedEntries> = LazyLock::new(|| {
    NamedEntries::bake(&test_corpus().0.0, test_registries(), test_item_table()).0
});

pub fn inputs<'i, 'c, W>(
    named: &'i NamedEntries,
    context: &'i LootContext<'c, ()>,
    world: &'i W,
) -> LootInputs<'i, 'c, (), W> {
    LootInputs {
        named,
        context,
        world,
    }
}

fn fresh() -> XoroshiroRandom {
    XoroshiroRandom::new(SEED)
}

fn empty_context() -> LootContext<'static, ()> {
    LootContext::try_new(ContextKeySet::Empty).expect("a source key set")
}

fn bake<T>(
    registries: &Registries,
    providers: &NamedEntries,
    json: &str,
    bake: impl FnOnce(&mut ConditionBaker, &str) -> T,
) -> (T, Vec<ConditionProblem>) {
    let mut baker = ConditionBaker::new(&test_corpus().0.0, registries, providers);
    let baked = bake(&mut baker, json);
    let problems = baker.drain_problems().collect();
    (baked, problems)
}

fn int(json: &str) -> IntProvider {
    let (provider, problems) = bake(test_registries(), &VANILLA, json, |baker, json| {
        baker.int_provider(&serde_json::from_str(json).expect("an int provider"))
    });
    assert_eq!(problems, [], "{json}");
    provider
}

fn float(json: &str) -> FloatProvider {
    let (provider, problems) = bake(test_registries(), &VANILLA, json, |baker, json| {
        baker.float_provider(&serde_json::from_str(json).expect("a float provider"))
    });
    assert_eq!(problems, [], "{json}");
    provider
}

fn read_int(json: &str, context: &LootContext<'_, ()>, random: &mut XoroshiroRandom) -> i32 {
    int(json).read(
        inputs(&VANILLA, context, &BlockHost::none()),
        random,
        &mut PredicateScratch::default(),
    )
}

fn read_float(json: &str, context: &LootContext<'_, ()>, random: &mut XoroshiroRandom) -> f32 {
    float(json).read(
        inputs(&VANILLA, context, &BlockHost::none()),
        random,
        &mut PredicateScratch::default(),
    )
}

const PASSES: &str = r#"{"type": "minecraft:survives_explosion"}"#;
const FAILS: &str =
    r#"{"type": "minecraft:inverted", "term": {"type": "minecraft:survives_explosion"}}"#;

#[test]
fn loot_num_004_to_019_int_types_give_their_values_and_errors() {
    let conditional = format!(
        r#"{{"type": "minecraft:conditional", "condition": {PASSES}, "on_true": 1, "on_false": 2}}"#
    );
    let conditional_absent =
        format!(r#"{{"type": "minecraft:conditional", "condition": {FAILS}, "on_true": 1}}"#);
    let dispatcher = format!(
        r#"{{"type": "minecraft:number_dispatcher", "cases": [
            {{"condition": {FAILS}, "value": 1}}, {{"condition": {PASSES}, "value": 2}}],
            "default": 3}}"#
    );
    let dispatcher_absent = format!(
        r#"{{"type": "minecraft:number_dispatcher", "cases": [{{"condition": {FAILS}, "value": 1}}]}}"#
    );
    let cases: [(&str, i32); 37] = [
        ("5", 5),
        ("3.0", 3),
        (r#"{"type": "minecraft:constant", "value": -4}"#, -4),
        (r#"{"type": "minecraft:abs", "input": -3}"#, 3),
        (r#"{"type": "minecraft:abs", "input": -2147483648}"#, 0),
        (r#"{"type": "minecraft:negate", "input": 4}"#, -4),
        (r#"{"type": "minecraft:negate", "input": -2147483648}"#, 0),
        (r#"{"type": "minecraft:from_float", "input": -2.9}"#, -2),
        (r#"{"type": "minecraft:from_float", "input": 3e9}"#, 0),
        (r#"{"type": "minecraft:sub", "left": 1, "right": 3}"#, -2),
        (
            r#"{"type": "minecraft:sub", "left": -2147483648, "right": 1}"#,
            0,
        ),
        (r#"{"type": "minecraft:mod", "left": -7, "right": 2}"#, -1),
        (
            r#"{"type": "minecraft:mod", "left": -2147483648, "right": -1}"#,
            0,
        ),
        (r#"{"type": "minecraft:mod", "left": 7, "right": 0}"#, 0),
        (
            r#"{"type": "minecraft:floor_mod", "left": -7, "right": 2}"#,
            1,
        ),
        (r#"{"type": "minecraft:div", "left": -7, "right": 2}"#, -3),
        (
            r#"{"type": "minecraft:div", "left": -2147483648, "right": -1}"#,
            -2147483648,
        ),
        (
            r#"{"type": "minecraft:floor_div", "left": -7, "right": 2}"#,
            -4,
        ),
        (
            r#"{"type": "minecraft:floor_div", "left": -2147483648, "right": -1}"#,
            0,
        ),
        (
            r#"{"type": "minecraft:pow", "base": -2, "exponent": 31}"#,
            -2147483648,
        ),
        (r#"{"type": "minecraft:pow", "base": 2, "exponent": 31}"#, 0),
        (r#"{"type": "minecraft:pow", "base": 0, "exponent": 0}"#, 0),
        (r#"{"type": "minecraft:pow", "base": 2, "exponent": -1}"#, 0),
        (r#"{"type": "minecraft:add", "inputs": [1, 2, 3]}"#, 6),
        (
            r#"{"type": "minecraft:add", "inputs": [1, {"type": "minecraft:div", "left": 1, "right": 0}]}"#,
            0,
        ),
        (r#"{"type": "minecraft:add", "inputs": [2147483647, 1]}"#, 0),
        (r#"{"type": "minecraft:mul", "inputs": [65536, 65536]}"#, 0),
        (r#"{"type": "minecraft:min", "inputs": [3, 1, 2]}"#, 1),
        (r#"{"type": "minecraft:max", "inputs": [3, 1, 2]}"#, 3),
        (r#"{"type": "minecraft:avg", "inputs": [-1, -2]}"#, -1),
        (&conditional, 1),
        (&conditional_absent, 0),
        (&dispatcher, 2),
        (&dispatcher_absent, 0),
        (
            r#"{"type": "minecraft:storage", "storage": "t:s", "path": "a.b", "fallback": 7}"#,
            7,
        ),
        (
            r#"{"type": "minecraft:score", "target": "this", "score": "s", "fallback": 4}"#,
            4,
        ),
        (
            r#"{"type": "minecraft:score", "target": {"type": "minecraft:fixed", "name": "N"}, "score": "s"}"#,
            0,
        ),
    ];
    let context = empty_context();
    for (json, value) in cases {
        let mut random = fresh();
        assert_eq!(read_int(json, &context, &mut random), value, "{json}");
        assert_eq!(random, fresh(), "the draws of {json}");
    }
}

#[test]
fn loot_num_005_to_021_float_types_give_their_values_and_errors() {
    let nan = r#"{"type": "div", "left": 0, "right": 0}"#;
    let ceil_nan = format!(r#"{{"type": "ceil", "input": {nan}}}"#);
    let round_nan = format!(r#"{{"type": "round", "input": {nan}}}"#);
    let level = r#"{"type": "enchantment_level", "amount":
        {"type": "linear", "base": 1, "per_level_above_first": 2}}"#;
    let cases: [(&str, Option<i32>, f32); 33] = [
        ("1.5", None, 1.5),
        (r#"{"type": "abs", "input": -0.5}"#, None, 0.5),
        (r#"{"type": "negate", "input": 0.0}"#, None, -0.0),
        (r#"{"type": "floor", "input": -1.5}"#, None, -2.0),
        (r#"{"type": "ceil", "input": 1.2}"#, None, 2.0),
        (&ceil_nan, None, 0.0),
        (r#"{"type": "round", "input": 2.5}"#, None, 3.0),
        (r#"{"type": "round", "input": -2.5}"#, None, -2.0),
        (&round_nan, None, 0.0),
        (r#"{"type": "truncate", "input": -0.5}"#, None, -0.0),
        (r#"{"type": "truncate", "input": 1.7}"#, None, 1.0),
        (r#"{"type": "sqrt", "input": 4}"#, None, 2.0),
        (r#"{"type": "sqrt", "input": -1}"#, None, 0.0),
        (r#"{"type": "cos", "input": 0}"#, None, 1.0),
        (r#"{"type": "from_int", "input": 3}"#, None, 3.0),
        (
            r#"{"type": "from_int", "input": {"type": "div", "left": 1, "right": 0}}"#,
            None,
            0.0,
        ),
        (r#"{"type": "sub", "left": 1, "right": 0.25}"#, None, 0.75),
        (r#"{"type": "div", "left": 1, "right": 4}"#, None, 0.25),
        (r#"{"type": "div", "left": 1, "right": 0}"#, None, 0.0),
        (r#"{"type": "mod", "left": -5.5, "right": 2}"#, None, -1.5),
        (r#"{"type": "mod", "left": 5.5, "right": -0.0}"#, None, 0.0),
        (r#"{"type": "pow", "base": 2, "exponent": 3}"#, None, 8.0),
        (r#"{"type": "pow", "base": -0.0, "exponent": 0}"#, None, 0.0),
        (r#"{"type": "add", "inputs": [0.5, 0.25]}"#, None, 0.75),
        (r#"{"type": "mul", "inputs": [2, 3]}"#, None, 6.0),
        (r#"{"type": "min", "inputs": [0.0, -0.0]}"#, None, -0.0),
        (r#"{"type": "max", "inputs": [-0.0, 0.0]}"#, None, 0.0),
        (r#"{"type": "avg", "inputs": [1, 2]}"#, None, 1.5),
        (r#"{"type": "length", "inputs": [3, 4]}"#, None, 5.0),
        (
            r#"{"type": "storage", "storage": "t:s", "path": "a", "fallback": 0.5}"#,
            None,
            0.5,
        ),
        (level, Some(3), 5.0),
        (level, None, -1.0),
        (level, Some(0), -1.0),
    ];
    for (json, level, value) in cases {
        let mut context = empty_context();
        context.enchantment_level = level;
        let mut random = fresh();
        let read = read_float(json, &context, &mut random);
        assert_eq!(read.to_bits(), value.to_bits(), "{json}: {read}");
        assert_eq!(random, fresh(), "the draws of {json}");
    }
}

#[test]
fn loot_num_007_binary_operands_draw_in_their_order() {
    let mut random = fresh();
    let first = random.next_f32();
    let second = random.next_f32();
    let two_draws = random.clone();
    let cases: [(&str, f32, XoroshiroRandom); 3] = [
        (
            r#"{"type": "minecraft:sub",
                "left": {"type": "minecraft:uniform", "min": 0, "max": 1},
                "right": {"type": "minecraft:uniform", "min": 10, "max": 11}}"#,
            first - (second + 10.0),
            two_draws.clone(),
        ),
        (
            r#"{"type": "minecraft:mod",
                "left": {"type": "minecraft:uniform", "min": 0, "max": 1},
                "right": {"type": "minecraft:uniform", "min": 2, "max": 3}}"#,
            second % (first + 2.0),
            two_draws,
        ),
        (
            r#"{"type": "minecraft:mod",
                "left": {"type": "minecraft:uniform", "min": 0, "max": 1},
                "right": 0}"#,
            0.0,
            fresh(),
        ),
    ];
    let context = empty_context();
    for (json, value, after) in cases {
        let mut random = fresh();
        assert_eq!(read_float(json, &context, &mut random), value, "{json}");
        assert_eq!(random, after, "the draws of {json}");
    }
}

#[test]
fn loot_num_001_an_error_takes_no_later_draw() {
    let json = r#"{"type": "minecraft:add", "inputs": [
        {"type": "minecraft:div", "left": 1, "right": 0},
        {"type": "minecraft:uniform", "min": 0, "max": 9}]}"#;
    let mut random = fresh();
    assert_eq!(read_int(json, &empty_context(), &mut random), 0);
    assert_eq!(random, fresh());
}

#[test]
fn loot_num_015_drawing_types_take_their_draws() {
    let weighted = r#"{"type": "minecraft:weighted_list", "distribution": [
        {"data": 10, "weight": 1},
        {"data": 0, "weight": 0},
        {"data": {"type": "minecraft:uniform", "min": 20, "max": 29}, "weight": 3}]}"#;
    let binomial = r#"{"type": "minecraft:binomial", "n": 3, "p": 0.5}"#;
    let conditional = r#"{"type": "minecraft:conditional",
        "condition": {"type": "minecraft:random_chance", "chance": 0.5},
        "on_true": {"type": "minecraft:uniform", "min": 0, "max": 9}, "on_false": 100}"#;
    let context = empty_context();
    let mut selected = [false; 2];
    for seed in 0..16 {
        let mut expected = XoroshiroRandom::new(seed);
        let k = bounded(&mut expected, 4).expect("a bound of 4");
        selected[usize::from(k >= 1)] = true;
        let value = match k {
            0 => 10,
            _ => 20 + bounded(&mut expected, 10).expect("a bound of 10"),
        };
        let mut random = XoroshiroRandom::new(seed);
        assert_eq!(
            read_int(weighted, &context, &mut random),
            value,
            "seed {seed}"
        );
        assert_eq!(random, expected, "the draws of seed {seed}");

        let mut expected = XoroshiroRandom::new(seed);
        let value: i32 = (0..3).map(|_| i32::from(expected.next_f32() < 0.5)).sum();
        let mut random = XoroshiroRandom::new(seed);
        assert_eq!(
            read_int(binomial, &context, &mut random),
            value,
            "seed {seed}"
        );
        assert_eq!(random, expected, "the draws of seed {seed}");

        let mut expected = XoroshiroRandom::new(seed);
        let value = match expected.next_f32() < 0.5 {
            true => bounded(&mut expected, 10).expect("a bound of 10"),
            false => 100,
        };
        let mut random = XoroshiroRandom::new(seed);
        assert_eq!(
            read_int(conditional, &context, &mut random),
            value,
            "seed {seed}"
        );
        assert_eq!(random, expected, "the draws of seed {seed}");
    }
    assert_eq!(selected, [true, true]);
}

#[test]
fn loot_num_016_a_positional_attribute_reads_the_origin() {
    let positional = float(
        r#"{"type": "minecraft:environment_attribute",
            "attribute": "minecraft:gameplay/turtle_egg_hatch_chance"}"#,
    );
    let global = float(
        r#"{"type": "minecraft:environment_attribute",
            "attribute": "minecraft:gameplay/sky_light_level"}"#,
    );
    assert!(positional.reads().contains(ContextParam::Origin));
    assert!(!global.reads().contains(ContextParam::Origin));
}

#[test]
fn reg_fld_722_the_decode_is_strict() {
    let int_cases = [
        r#"{"type": "minecraft:unknown"}"#,
        "1.5",
        "2147483648",
        "true",
        r#"{"type": "minecraft:abs"}"#,
        r#"{"type": "minecraft:weighted_list", "distribution": []}"#,
        r#"{"type": "minecraft:weighted_list", "distribution": [{"data": 1, "weight": -1}]}"#,
        r#"{"type": "minecraft:weighted_list", "distribution": [{"data": 1, "weight": 0}]}"#,
        r#"{"type": "minecraft:weighted_list", "distribution":
            [{"data": 1, "weight": 2147483647}, {"data": 2, "weight": 1}]}"#,
        r#"{"type": "minecraft:environment_attribute",
            "attribute": "minecraft:gameplay/turtle_egg_hatch_chance"}"#,
    ];
    for json in int_cases {
        assert!(
            serde_json::from_str::<IntProviderFile>(json).is_err(),
            "{json}"
        );
    }
    let float_cases = [
        r#"{"type": "minecraft:unknown"}"#,
        r#"{"type": "minecraft:environment_attribute", "attribute": "minecraft:visual/fog_color"}"#,
        r#"{"type": "minecraft:enchantment_level",
            "amount": {"type": "minecraft:clamped", "value": 1, "min": 2, "max": 1}}"#,
    ];
    for json in float_cases {
        assert!(
            serde_json::from_str::<FloatProviderFile>(json).is_err(),
            "{json}"
        );
    }
    for json in [
        r#""minecraft:any""#,
        r##"{"type": "add", "inputs": "#t:g"}"##,
    ] {
        serde_json::from_str::<IntProviderFile>(json).expect(json);
    }
}

fn int_problems(
    registries: &Registries,
    providers: &NamedEntries,
    json: &str,
) -> Vec<ConditionProblem> {
    let (provider, problems) = bake(registries, providers, json, |baker, json| {
        baker.int_provider(&serde_json::from_str(json).expect("an int provider"))
    });
    assert_eq!(
        provider.read(
            inputs(providers, &empty_context(), &BlockHost::none()),
            &mut fresh(),
            &mut PredicateScratch::default()
        ),
        0
    );
    problems
}

fn nested(depth: usize) -> String {
    format!(
        "{}1{}",
        r#"{"type": "minecraft:abs", "input": "#.repeat(depth),
        "}".repeat(depth)
    )
}

#[test]
fn loot_ctx_017_a_chain_has_at_most_64_members() {
    let (_, problems) = bake(test_registries(), &VANILLA, &nested(63), |baker, json| {
        baker.int_provider(&serde_json::from_str(json).expect("an int provider"))
    });
    assert_eq!(problems, []);
    let problems = int_problems(test_registries(), &VANILLA, &nested(64));
    let [ConditionProblem::ChainTooLong(path)] = problems.as_slice() else {
        panic!("{problems:?}");
    };
    assert_eq!(path.split(" > ").count(), 64, "{path}");
}

static PACK: LazyLock<(Registries, NamedEntries, Vec<NamedIssue>)> = LazyLock::new(|| {
    let mut additions = test_additions();
    let mut files = vec![
        (
            "a".to_owned(),
            r#"{"type": "minecraft:abs", "input": "t:b"}"#.to_owned(),
        ),
        (
            "b".to_owned(),
            r#"{"type": "minecraft:negate", "input": "t:a"}"#.to_owned(),
        ),
        ("one".to_owned(), "1".to_owned()),
        ("d/13".to_owned(), "1".to_owned()),
        (
            "c".to_owned(),
            r#"{"type": "minecraft:conditional", "condition": "t:p", "on_true": 1}"#.to_owned(),
        ),
    ];
    for level in 0..13 {
        let next = level + 1;
        let add = format!(r#"{{"type": "minecraft:add", "inputs": ["t:d/{next}", "t:d/{next}"]}}"#);
        files.push((format!("d/{level}"), add));
    }
    for (name, text) in files {
        additions.files.insert(
            "pack",
            format!("data/t/context_int_provider/{name}.json"),
            text,
        );
    }
    let mut predicates = vec![
        (
            "x".to_owned(),
            r#"{"type": "minecraft:inverted", "term": "t:y"}"#.to_owned(),
        ),
        (
            "y".to_owned(),
            r#"{"type": "minecraft:inverted", "term": "t:x"}"#.to_owned(),
        ),
        (
            "p".to_owned(),
            r#"{"type": "minecraft:int_value_check", "value": "t:c", "test": 0}"#.to_owned(),
        ),
    ];
    for (chain, length) in [("chain64", 64), ("chain65", 65), ("chain66", 66)] {
        for member in 0..length {
            let next = member + 1;
            let text = match next == length {
                true => r#"{"type": "minecraft:survives_explosion"}"#.to_owned(),
                false => format!(r#"{{"type": "minecraft:inverted", "term": "t:{chain}/{next}"}}"#),
            };
            predicates.push((format!("{chain}/{member}"), text));
        }
    }
    for (name, text) in predicates {
        additions
            .files
            .insert("pack", format!("data/t/predicate/{name}.json"), text);
    }
    additions.embedded_packs.push("pack".to_owned());
    let registries = load_registries(&test_packs(), additions)
        .expect("the pack loads")
        .0;
    let (providers, issues) =
        NamedEntries::bake(&test_corpus().0.0, &registries, test_item_table());
    (registries, providers, issues)
});

#[test]
fn loot_ctx_014_a_cycle_and_a_missing_key_are_faults() {
    let (registries, providers, issues) = &*PACK;
    let mut found: Vec<(&str, MemberKind, &ConditionProblem)> = issues
        .iter()
        .filter(|issue| !issue.entry.as_str().starts_with("t:chain"))
        .map(|issue| (issue.entry.as_str(), issue.kind, &issue.problem))
        .collect();
    found.sort_by_key(|(entry, ..)| *entry);
    let cycle = |path: &str| ConditionProblem::Cycle(path.to_owned());
    assert_eq!(
        found,
        [
            ("t:a", MemberKind::IntProvider, &cycle("t:a > t:b > t:a")),
            ("t:b", MemberKind::IntProvider, &cycle("t:b > t:a > t:b")),
            ("t:c", MemberKind::IntProvider, &cycle("t:c > t:p > t:c")),
            ("t:p", MemberKind::Predicate, &cycle("t:p > t:c > t:p")),
            ("t:x", MemberKind::Predicate, &cycle("t:x > t:y > t:x")),
            ("t:y", MemberKind::Predicate, &cycle("t:y > t:x > t:y")),
        ]
    );
    for key in ["t:x", "t:y"] {
        let json = format!(r#""{key}""#);
        let (condition, _) = bake(registries, providers, &json, |baker, json| {
            baker.condition(&serde_json::from_str(json).expect("a condition"))
        });
        let context = empty_context();
        let passed = condition.test(
            inputs(providers, &context, &BlockHost::none()),
            &mut fresh(),
            &mut PredicateScratch::default(),
        );
        assert!(!passed, "{key}");
    }
    let cases: [(&str, ConditionProblem); 2] = [
        (
            r#""t:missing""#,
            ConditionProblem::UnknownProvider("t:missing".to_owned()),
        ),
        (
            r#"{"type": "minecraft:abs", "input": {"type": "minecraft:add", "inputs": []}}"#,
            ConditionProblem::EmptyInputs("input".to_owned()),
        ),
    ];
    for (json, problem) in cases {
        assert_eq!(
            int_problems(registries, providers, json),
            [problem],
            "{json}"
        );
    }
}

#[test]
fn loot_ctx_017_a_chain_of_predicates_has_at_most_64_members() {
    let (_, _, issues) = &*PACK;
    let mut found: Vec<(&str, MemberKind, bool)> = issues
        .iter()
        .filter(|issue| issue.entry.as_str().starts_with("t:chain"))
        .map(|issue| {
            let chain = matches!(issue.problem, ConditionProblem::ChainTooLong(_));
            (issue.entry.as_str(), issue.kind, chain)
        })
        .collect();
    found.sort_by_key(|(entry, ..)| *entry);
    assert_eq!(
        found,
        [
            ("t:chain65/0", MemberKind::Predicate, true),
            ("t:chain66/0", MemberKind::Predicate, true),
            ("t:chain66/1", MemberKind::Predicate, true),
        ]
    );
}

#[test]
fn loot_ctx_017_a_key_refers_to_one_baked_entry() {
    let (registries, providers, _) = &*PACK;
    let uses = vec![r#""t:one""#; 4097].join(", ");
    let cases = [
        (
            format!(r#"{{"type": "minecraft:add", "inputs": [{uses}]}}"#),
            4097,
        ),
        (r#""t:d/0""#.to_owned(), 8192),
    ];
    for (json, value) in cases {
        let (provider, problems) = bake(registries, providers, &json, |baker, json| {
            baker.int_provider(&serde_json::from_str(json).expect("an int provider"))
        });
        assert_eq!(problems, [], "{json}");
        let context = empty_context();
        let read = provider.read(
            inputs(providers, &context, &BlockHost::none()),
            &mut fresh(),
            &mut PredicateScratch::default(),
        );
        assert_eq!(read, value, "{json}");
    }
}

#[test]
fn reg_fld_722_the_vanilla_named_entries_bake() {
    let registries = test_registries();
    assert!(
        registries
            .registry::<IntProviderFile>()
            .is_some_and(|ints| !ints.is_empty())
    );
    assert!(
        registries
            .registry::<FloatProviderFile>()
            .is_some_and(|floats| !floats.is_empty())
    );
    let issues = NamedEntries::bake(&test_corpus().0.0, registries, test_item_table()).1;
    assert_eq!(issues, []);
}

#[test]
fn loot_num_010_a_uniform_draws_only_for_a_range() {
    let mut random = fresh();
    let draw = random.next_f32();
    let one_float = random;
    let floats = [
        (
            r#"{"type": "minecraft:uniform", "min": 2.0, "max": 1.0}"#,
            2.0,
            fresh(),
        ),
        (
            r#"{"type": "minecraft:uniform", "min": 1.5, "max": 1.5}"#,
            1.5,
            fresh(),
        ),
        (
            r#"{"type": "minecraft:uniform", "min": 1.0, "max": 3.0}"#,
            draw * 2.0 + 1.0,
            one_float,
        ),
    ];
    let ints = [
        (r#"{"type": "minecraft:uniform", "min": 3, "max": 3}"#, 3),
        (
            r#"{"type": "minecraft:uniform", "min": -2147483648, "max": 2147483647}"#,
            0,
        ),
    ];
    let context = empty_context();
    for (json, value, after) in floats {
        let mut random = fresh();
        assert_eq!(read_float(json, &context, &mut random), value, "{json}");
        assert_eq!(random, after, "the draws of {json}");
    }
    for (json, value) in ints {
        let mut random = fresh();
        assert_eq!(read_int(json, &context, &mut random), value, "{json}");
        assert_eq!(random, fresh(), "the draws of {json}");
    }
}
