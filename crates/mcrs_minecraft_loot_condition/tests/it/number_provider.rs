use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{ConditionBaker, LootContext, NumberFile};
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

enum Read {
    Int,
    Float,
}

/// LOOT-NUM-009, LOOT-NUM-010, LOOT-NUM-001: a `uniform` takes its draw only for `min < max`,
/// and an int `uniform` whose range is too wide for `B` reads as 0 without a draw.
#[test]
fn loot_num_010_a_uniform_draws_only_for_a_range() {
    let fresh = || XoroshiroRandom::new(42);
    let after_one_float = || {
        let mut random = fresh();
        let draw = random.next_f32();
        (draw, random)
    };
    let (draw, one_float) = after_one_float();
    let cases = [
        (
            r#"{"type": "minecraft:uniform", "min": 2.0, "max": 1.0}"#,
            Read::Float,
            2.0,
            fresh(),
        ),
        (
            r#"{"type": "minecraft:uniform", "min": 1.5, "max": 1.5}"#,
            Read::Float,
            1.5,
            fresh(),
        ),
        (
            r#"{"type": "minecraft:uniform", "min": 1.0, "max": 3.0}"#,
            Read::Float,
            draw * 2.0 + 1.0,
            one_float,
        ),
        (
            r#"{"type": "minecraft:uniform", "min": 3, "max": 3}"#,
            Read::Int,
            3.0,
            fresh(),
        ),
        (
            r#"{"type": "minecraft:uniform", "min": -2147483648, "max": 2147483647}"#,
            Read::Int,
            0.0,
            fresh(),
        ),
    ];
    let mut baker = ConditionBaker::new(&test_corpus().0.0, test_registries());
    let context: LootContext<'_, ()> =
        LootContext::try_new(ContextKeySet::Empty).expect("a source key set");
    for (json, read, value, after) in cases {
        let file: NumberFile = serde_json::from_str(json).expect("a provider");
        let provider = baker.number(&file);
        let mut random = fresh();
        let read_value = match read {
            Read::Int => f64::from(provider.read_int(&context, &NoWorld, &mut random)),
            Read::Float => f64::from(provider.read_float(&context, &NoWorld, &mut random)),
        };
        assert_eq!(read_value, f64::from(value), "{json}");
        assert_eq!(random, after, "the draws of {json}");
    }
}
