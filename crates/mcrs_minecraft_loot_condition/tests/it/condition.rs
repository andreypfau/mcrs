use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    ConditionBaker, ConditionFile, ContextParam, LootContext, ParamNotAllowed,
};
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

/// LOOT-CTX-013: a condition reads only the parameters that the type of its table allows.
#[test]
fn loot_ctx_013_a_condition_reads_only_allowed_parameters() {
    let cases = [
        (
            r#"{"type": "minecraft:survives_explosion"}"#,
            ContextKeySet::Block,
            None,
        ),
        (
            r#"{"type": "minecraft:survives_explosion"}"#,
            ContextKeySet::Chest,
            Some(ContextParam::ExplosionRadius),
        ),
        (
            r#"{"type": "minecraft:match_tool"}"#,
            ContextKeySet::Entity,
            Some(ContextParam::Tool),
        ),
        (
            r#"{"type": "minecraft:location_check"}"#,
            ContextKeySet::Barter,
            Some(ContextParam::Origin),
        ),
        (
            r#"{"type": "minecraft:inverted", "term": {"type": "minecraft:match_block"}}"#,
            ContextKeySet::BlockUse,
            None,
        ),
        (
            r#"{"type": "minecraft:any_of", "terms": [{"type": "minecraft:random_chance", "chance": 0.5}, {"type": "minecraft:match_block"}]}"#,
            ContextKeySet::Fishing,
            Some(ContextParam::BlockState),
        ),
        (
            r#"{"type": "minecraft:random_chance", "chance": 0.5}"#,
            ContextKeySet::Empty,
            None,
        ),
    ];
    let mut baker = ConditionBaker::new(&test_corpus().0.0, test_registries());
    for (json, key_set, refused) in cases {
        let file: ConditionFile = serde_json::from_str(json).expect("a condition");
        let reads = baker.condition(&file).reads();
        let expected = refused.map_or(Ok(()), |param| Err(ParamNotAllowed { param, key_set }));
        assert_eq!(reads.check(key_set), expected, "{json}");
    }
}

/// LOOT-CND-004, LOOT-CND-007, LOOT-CND-009: the result and the draws of a condition in a context
/// without `this_entity` and without `tool`.
#[test]
fn loot_cnd_a_condition_without_its_parameter() {
    let fresh = || XoroshiroRandom::new(42);
    let after_one_float = || {
        let mut random = fresh();
        random.next_f32();
        random
    };
    let cases = [
        (
            "LOOT-CND-009",
            r#"{"type": "minecraft:entity_properties", "entity": "this"}"#,
            true,
            fresh(),
        ),
        (
            "LOOT-CND-009",
            r#"{"type": "minecraft:entity_properties", "entity": "attacker"}"#,
            true,
            fresh(),
        ),
        (
            "LOOT-CND-004",
            r#"{"type": "minecraft:table_bonus", "enchantment": "minecraft:fortune", "chances": [0.0, 1.0]}"#,
            false,
            after_one_float(),
        ),
        (
            "LOOT-CND-007",
            r#"{"type": "minecraft:match_tool"}"#,
            false,
            fresh(),
        ),
    ];
    let mut baker = ConditionBaker::new(&test_corpus().0.0, test_registries());
    let context: LootContext<'_, ()> =
        LootContext::try_new(ContextKeySet::Empty).expect("a source key set");
    for (rule, json, passes, after) in cases {
        let file: ConditionFile = serde_json::from_str(json).expect("a condition");
        let condition = baker.condition(&file);
        let mut random = fresh();
        assert_eq!(
            condition.test(&context, &NoWorld, &mut random),
            passes,
            "{rule}: {json}"
        );
        assert_eq!(random, after, "{rule}: the draws of {json}");
    }
}
