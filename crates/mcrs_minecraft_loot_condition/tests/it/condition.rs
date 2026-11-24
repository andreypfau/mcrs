use std::num::NonZeroU8;

use glam::DVec3;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_environment::{AttributeType, AttributeValue};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{
    Count, Damage, EnchantmentId, EnchantmentLevel, Enchantments, ItemStack, Kind,
};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    Condition, ConditionBaker, ConditionFile, ContextParam, LootContext, LootInputs,
    ParamNotAllowed, Tool,
};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_predicate::{BlockHost, DamageSource, PredicateScratch};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::{Id, ResourceLocation};
use mcrs_voxel::BlockStateId;

use crate::host::{Mob, Mobs};
use crate::number_provider::{VANILLA, inputs};

pub fn condition(json: &str) -> Condition {
    let mut baker = ConditionBaker::new(&test_corpus().0.0, test_registries(), &VANILLA);
    let file: ConditionFile = serde_json::from_str(json).expect(json);
    let condition = baker.condition(&file);
    assert_eq!(baker.drain_problems().collect::<Vec<_>>(), [], "{json}");
    condition
}

pub fn empty_context<'a>() -> LootContext<'a, ()> {
    LootContext::try_new(ContextKeySet::Empty).expect("a source key set")
}

fn passes(condition: &Condition, context: &LootContext<'_, ()>) -> bool {
    condition.test(
        inputs(&VANILLA, context, &BlockHost::none()),
        &mut XoroshiroRandom::new(42),
        &mut PredicateScratch::default(),
    )
}

pub fn item(name: &str) -> Kind {
    let id = test_registries()
        .entries::<Item>()
        .and_then(|items| items.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla item");
    Kind(id.number())
}

fn default_state(block: &str) -> BlockStateId {
    test_corpus()
        .0
        .0
        .block(block)
        .expect("a vanilla block")
        .default_state_id
}

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
    for (json, key_set, refused) in cases {
        let reads = condition(json).reads();
        let expected = refused.map_or(Ok(()), |param| Err(ParamNotAllowed { param, key_set }));
        assert_eq!(reads.check(key_set), expected, "{json}");
    }
}

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
    let context = empty_context();
    for (rule, json, passes, after) in cases {
        let mut random = fresh();
        let result = condition(json).test(
            inputs(&VANILLA, &context, &BlockHost::none()),
            &mut random,
            &mut PredicateScratch::default(),
        );
        assert_eq!(result, passes, "{rule}: {json}");
        assert_eq!(random, after, "{rule}: the draws of {json}");
    }
}

#[test]
fn reg_fld_006_the_decode_of_a_condition_is_strict_on_types() {
    let cases = [
        (r#"{"type": "minecraft:no_such_condition"}"#, false),
        (
            r#"{"type": "minecraft:survives_explosion", "zzz": 1}"#,
            true,
        ),
        (r#"{"type": "minecraft:match_tool", "zzz": 1}"#, true),
        (
            r#"{"type": "minecraft:entity_properties", "entity": "this", "predicate": {"zzz": 1}}"#,
            false,
        ),
        (
            r#"{"type": "minecraft:entity_properties", "entity": "this", "predicate": {"flags": {"zzz": 1}}}"#,
            true,
        ),
        (
            r#"{"type": "minecraft:match_block", "blocks": "minecraft:stone", "zzz": 1}"#,
            true,
        ),
        (
            r#"{"type": "minecraft:match_tool", "predicate": {"zzz": 1}}"#,
            true,
        ),
        (
            r#"{"type": "minecraft:match_tool", "predicate": {"predicates": {"minecraft:damage": {"durability": {"min": 1}, "zzz": 1}}}}"#,
            true,
        ),
        (
            r#"{"type": "minecraft:location_check", "predicate": {"block": {"zzz": 1}}}"#,
            true,
        ),
    ];
    for (json, decodes) in cases {
        let decoded = serde_json::from_str::<ConditionFile>(json);
        assert_eq!(decoded.is_ok(), decodes, "{json}: {decoded:?}");
    }
}

#[test]
fn reg_fld_719_720_a_value_and_a_range_decode_by_their_form() {
    let attribute = |name: &str, value: &str| {
        format!(
            r#"{{"type": "minecraft:environment_attribute_check", "attribute": "minecraft:{name}", "value": {value}}}"#
        )
    };
    let range = |test: &str| {
        format!(r#"{{"type": "minecraft:int_value_check", "value": 1, "test": {test}}}"#)
    };
    let cases = [
        (attribute("visual/fog_end_distance", "1"), true),
        (attribute("visual/fog_end_distance", "true"), false),
        (attribute("gameplay/water_evaporates", "true"), true),
        (attribute("gameplay/water_evaporates", "1"), false),
        (range(r#"{"type": "minecraft:constant", "value": 1}"#), true),
        (range(r#"{"min": 1}"#), true),
        (range(r#"{"type": "minecraft:not_a_provider"}"#), false),
        (
            range(r#"{"type": "minecraft:not_a_provider", "min": 1}"#),
            false,
        ),
    ];
    for (json, decodes) in cases {
        let decoded = serde_json::from_str::<ConditionFile>(&json);
        assert_eq!(decoded.is_ok(), decodes, "{json}: {decoded:?}");
    }
}

#[test]
fn loot_cnd_006_match_block_reads_the_block_entity() {
    let match_chest = condition(
        r#"{"type": "minecraft:match_block", "blocks": "minecraft:chest", "nbt": "{CustomName: \"x\"}"}"#,
    );
    let mut named = NbtCompound::new();
    named.put_string("CustomName", "x".to_owned());
    let unnamed = NbtCompound::new();
    let chest = default_state("minecraft:chest");
    let stone = default_state("minecraft:stone");
    let cases = [
        (Some(chest), Some(&named), true),
        (Some(chest), Some(&unnamed), false),
        (Some(chest), None, false),
        (Some(stone), Some(&named), false),
        (None, Some(&named), false),
    ];
    for (state, block_entity, expected) in cases {
        let mut context = empty_context();
        context.block_state = state;
        context.block_entity = block_entity;
        assert_eq!(
            passes(&match_chest, &context),
            expected,
            "{state:?} {block_entity:?}"
        );
    }
}

#[test]
fn loot_cnd_007_match_tool_matches_components() {
    let worn_pickaxe = condition(
        r#"{"type": "minecraft:match_tool", "predicate": {"items": "minecraft:diamond_pickaxe", "components": {"minecraft:damage": 3}}}"#,
    );
    let pickaxe = ItemStack::new(
        item("minecraft:diamond_pickaxe"),
        Count::new(1).expect("a count of 1"),
    );
    let worn = pickaxe.clone().set(Damage(3));
    let cases = [
        (Some(Tool::Stack(&worn)), true),
        (Some(Tool::Stack(&pickaxe)), false),
        (Some(Tool::Empty), false),
        (None, false),
    ];
    for (tool, expected) in cases {
        let mut context = empty_context();
        context.tool = tool;
        assert_eq!(passes(&worn_pickaxe, &context), expected, "{tool:?}");
    }
}

#[test]
fn loot_cnd_009_entity_properties_tests_its_target() {
    let any_this = r#"{"type": "minecraft:entity_properties", "entity": "this", "predicate": {}}"#;
    let any_attacker =
        r#"{"type": "minecraft:entity_properties", "entity": "attacker", "predicate": {}}"#;
    let pig = |target: &str| {
        format!(
            r#"{{"type": "minecraft:entity_properties", "entity": "{target}", "predicate": {{"entity_type": "minecraft:pig"}}}}"#
        )
    };
    let cases = [
        (any_this.to_owned(), Some(Mob::Pig), None, true),
        (any_this.to_owned(), None, Some(Mob::Pig), false),
        (any_attacker.to_owned(), None, Some(Mob::Pig), true),
        (any_attacker.to_owned(), Some(Mob::Pig), None, false),
        (pig("this"), Some(Mob::Pig), None, true),
        (pig("this"), Some(Mob::Zombie), None, false),
        (pig("this"), Some(Mob::Zombie), Some(Mob::Pig), false),
        (pig("attacker"), Some(Mob::Zombie), Some(Mob::Pig), true),
        (pig("attacker"), Some(Mob::Pig), Some(Mob::Zombie), false),
    ];
    for (json, this, attacker, expected) in cases {
        let mut context = mob_context();
        context.this_entity = this;
        context.attacking_entity = attacker;
        let (passed, _) = mob_test(&condition(&json), &context, &Mobs::default());
        assert_eq!(passed, expected, "{json} {this:?} {attacker:?}");
    }
}

fn mob_context<'a>() -> LootContext<'a, Mob> {
    LootContext::try_new(ContextKeySet::Empty).expect("a source key set")
}

fn mob_test(
    condition: &Condition,
    context: &LootContext<'_, Mob>,
    host: &Mobs<'_>,
) -> (bool, XoroshiroRandom) {
    let mut random = fresh();
    let inputs = LootInputs {
        named: &VANILLA,
        context,
        world: host,
    };
    let passed = condition.test(inputs, &mut random, &mut PredicateScratch::default());
    (passed, random)
}

const INT_DRAW: &str = r#"{"type": "minecraft:binomial", "n": 1, "p": 0.0}"#;

const FLOAT_DRAW: &str = r#"{"type": "minecraft:uniform", "min": 0.0, "max": 1.0}"#;

fn fresh() -> XoroshiroRandom {
    XoroshiroRandom::new(42)
}

fn after_floats(count: usize) -> XoroshiroRandom {
    let mut random = fresh();
    for _ in 0..count {
        random.next_f32();
    }
    random
}

fn enchanted_sword(enchantment: &str, level: u8) -> ItemStack {
    let id: Id<Enchantment> = test_registries()
        .entries::<Enchantment>()
        .and_then(|entries| entries.by_name(ResourceLocation::read(enchantment).ok()?))
        .expect("a vanilla enchantment");
    let enchantment = EnchantmentLevel {
        enchantment: EnchantmentId(id.number()),
        level: NonZeroU8::new(level).expect("a level above 0"),
    };
    ItemStack::new(
        item("minecraft:iron_sword"),
        Count::new(1).expect("a count of 1"),
    )
    .set(Enchantments::new([enchantment]).expect("one enchantment"))
}

#[test]
fn loot_cnd_010_the_chance_follows_the_level_on_the_attacker() {
    let bonus = |enchantment: &str| {
        condition(&format!(
            r#"{{"type": "minecraft:random_chance_with_enchanted_bonus", "enchantment": "{enchantment}", "unenchanted_chance": 0.0, "enchanted_chance": {{"type": "minecraft:linear", "base": -1.0, "per_level_above_first": 2.0}}}}"#
        ))
    };
    let looting = bonus("minecraft:looting");
    let protection = bonus("minecraft:protection");
    let first = enchanted_sword("minecraft:looting", 1);
    let second = enchanted_sword("minecraft:looting", 2);
    let protected = enchanted_sword("minecraft:protection", 2);
    let plain = ItemStack::new(
        item("minecraft:iron_sword"),
        Count::new(1).expect("a count of 1"),
    );
    let cases = [
        (&looting, None, Some(&second), false),
        (&looting, Some(Mob::Zombie), None, false),
        (&looting, Some(Mob::Zombie), Some(&plain), false),
        (&looting, Some(Mob::Zombie), Some(&first), false),
        (&looting, Some(Mob::Zombie), Some(&second), true),
        (&looting, Some(Mob::Arrow), Some(&second), false),
        (&protection, Some(Mob::Zombie), Some(&protected), false),
    ];
    for (bonus, attacker, weapon, expected) in cases {
        let mut context = mob_context();
        context.attacking_entity = attacker;
        let host = Mobs {
            weapon,
            ..Mobs::default()
        };
        let result = mob_test(bonus, &context, &host);
        assert_eq!(
            result,
            (expected, after_floats(1)),
            "{attacker:?} {weapon:?}"
        );
    }
}

#[test]
fn loot_cnd_011_018_a_condition_on_one_parameter() {
    let killed = r#"{"type": "minecraft:killed_by_player"}"#;
    let active = r#"{"type": "minecraft:enchantment_active_check", "active": true}"#;
    let inactive = r#"{"type": "minecraft:enchantment_active_check", "active": false}"#;
    let cases = [
        (killed, None, None, false),
        (killed, Some(Mob::Zombie), None, true),
        (active, None, None, false),
        (active, None, Some(true), true),
        (active, None, Some(false), false),
        (inactive, None, Some(false), true),
        (inactive, None, None, false),
    ];
    for (json, player, enchantment_active, expected) in cases {
        let mut context = mob_context();
        context.last_damage_player = player;
        context.enchantment_active = enchantment_active;
        let result = mob_test(&condition(json), &context, &Mobs::default());
        assert_eq!(
            result,
            (expected, fresh()),
            "{json} {player:?} {enchantment_active:?}"
        );
    }
}

#[test]
fn loot_cnd_012_entity_scores_stop_at_the_first_failure() {
    let scores = |entity: &str, scores: &str| {
        format!(
            r#"{{"type": "minecraft:entity_scores", "entity": "{entity}", "scores": {scores}}}"#
        )
    };
    let draw = format!(r#"{{"min": {INT_DRAW}}}"#);
    let cases = [
        (scores("this", "{}"), None, false, 0),
        (scores("this", "{}"), Some(Mob::Pig), true, 0),
        (
            scores("this", r#"{"kills": 5}"#),
            Some(Mob::Zombie),
            true,
            0,
        ),
        (
            scores("this", r#"{"kills": {"min": 3, "max": 5}}"#),
            Some(Mob::Zombie),
            true,
            0,
        ),
        (
            scores("this", r#"{"kills": {"min": 6}}"#),
            Some(Mob::Zombie),
            false,
            0,
        ),
        (
            scores("this", r#"{"kills": {"min": 3}}"#),
            Some(Mob::Pig),
            false,
            0,
        ),
        (
            scores("this", r#"{"missing": {}}"#),
            Some(Mob::Zombie),
            false,
            0,
        ),
        (
            scores("attacker", r#"{"kills": 5}"#),
            Some(Mob::Zombie),
            false,
            0,
        ),
        (
            scores("this", &format!(r#"{{"kills": {draw}}}"#)),
            Some(Mob::Zombie),
            true,
            1,
        ),
        (
            scores(
                "this",
                &format!(r#"{{"deaths": {{"min": 2}}, "kills": {draw}}}"#),
            ),
            Some(Mob::Zombie),
            false,
            0,
        ),
        (
            scores(
                "this",
                &format!(r#"{{"kills": {draw}, "deaths": {{"min": 2}}}}"#),
            ),
            Some(Mob::Zombie),
            false,
            1,
        ),
    ];
    let host = Mobs {
        scores: &[(Mob::Zombie, "kills", 5), (Mob::Zombie, "deaths", 1)],
        ..Mobs::default()
    };
    for (json, this, expected, draws) in cases {
        let mut context = mob_context();
        context.this_entity = this;
        let result = mob_test(&condition(&json), &context, &host);
        assert_eq!(result, (expected, after_floats(draws)), "{json} {this:?}");
    }
}

#[test]
fn loot_cnd_013_damage_source_properties_tests_the_source() {
    let check =
        |predicate: &str| format!(r#"{{"type": "minecraft:damage_source_properties"{predicate}}}"#);
    let zombie_source =
        check(r#", "predicate": {"source_entity": {"entity_type": "minecraft:zombie"}}"#);
    let pig_source = check(r#", "predicate": {"source_entity": {"entity_type": "minecraft:pig"}}"#);
    let arrow_direct =
        check(r#", "predicate": {"direct_entity": {"entity_type": "minecraft:arrow"}}"#);
    let arrow = damage_type("minecraft:arrow");
    let shot = Some(DamageSource {
        damage_type: arrow,
        direct_entity: Some(Mob::Arrow),
        causing_entity: Some(Mob::Zombie),
    });
    let cases = [
        (check(""), shot, Some(DVec3::ZERO), true),
        (check(""), None, Some(DVec3::ZERO), false),
        (check(""), shot, None, false),
        (zombie_source.clone(), shot, Some(DVec3::ZERO), true),
        (zombie_source, shot, None, false),
        (pig_source, shot, Some(DVec3::ZERO), false),
        (arrow_direct, shot, Some(DVec3::ZERO), true),
    ];
    for (json, source, origin, expected) in cases {
        let mut context = mob_context();
        context.damage_source = source;
        context.origin = origin;
        let result = mob_test(&condition(&json), &context, &Mobs::default());
        assert_eq!(result, (expected, fresh()), "{json} {source:?} {origin:?}");
    }
}

fn damage_type(name: &str) -> Id<DamageType> {
    test_registries()
        .entries::<DamageType>()
        .and_then(|entries| entries.by_name(ResourceLocation::read(name).ok()?))
        .expect("a vanilla damage type")
}

#[test]
fn loot_cnd_014_weather_check_compares_each_present_field() {
    let check = |fields: &str| format!(r#"{{"type": "minecraft:weather_check"{fields}}}"#);
    let raining = check(r#", "raining": true"#);
    let dry = check(r#", "raining": false"#);
    let thundering = check(r#", "thundering": true"#);
    let calm = check(r#", "raining": true, "thundering": false"#);
    let cases = [
        (check(""), None, None, true),
        (raining.clone(), None, None, false),
        (dry.clone(), None, None, true),
        (raining.clone(), Some(0.2), None, false),
        (raining, Some(0.25), None, true),
        (dry, Some(0.25), None, false),
        (thundering.clone(), Some(1.0), Some(0.95), true),
        (thundering.clone(), Some(0.9), Some(1.0), false),
        (thundering, None, Some(1.0), false),
        (calm.clone(), Some(1.0), Some(0.5), true),
        (calm, Some(1.0), Some(1.0), false),
    ];
    for (json, rain, thunder, expected) in cases {
        let host = Mobs {
            rain,
            thunder,
            ..Mobs::default()
        };
        let result = mob_test(&condition(&json), &mob_context(), &host);
        assert_eq!(result, (expected, fresh()), "{json} {rain:?} {thunder:?}");
    }
}

#[test]
fn loot_cnd_015_time_check_tests_the_ticks_of_the_clock() {
    let check = |period: &str, value: &str| {
        format!(
            r#"{{"type": "minecraft:time_check", "clock": "minecraft:overworld"{period}, "value": {value}}}"#
        )
    };
    let day = r#", "period": 24000"#;
    let cases = [
        (check("", "0"), None, true, 0),
        (check("", r#"{"min": 1}"#), None, false, 0),
        (check(day, r#"{"min": 13000}"#), Some(37_000), true, 0),
        (check(day, r#"{"min": 13000}"#), Some(36_000), false, 0),
        (check(day, "-1"), Some(-1), true, 0),
        (check(day, r#"{"min": 0}"#), Some(-24_001), false, 0),
        (check("", "5"), Some((1 << 32) + 5), true, 0),
        (check("", "-1"), Some(u32::MAX.into()), true, 0),
        (
            check("", &format!(r#"{{"min": {INT_DRAW}}}"#)),
            Some(7),
            true,
            1,
        ),
    ];
    for (json, clock, expected, draws) in cases {
        let host = Mobs {
            clock,
            ..Mobs::default()
        };
        let result = mob_test(&condition(&json), &mob_context(), &host);
        assert_eq!(result, (expected, after_floats(draws)), "{json} {clock:?}");
    }
}

#[test]
fn loot_cnd_016_017_a_value_check_reads_in_order() {
    let int = |value: &str, test: &str| {
        format!(r#"{{"type": "minecraft:int_value_check", "value": {value}, "test": {test}}}"#)
    };
    let float = |value: &str, test: &str| {
        format!(r#"{{"type": "minecraft:float_value_check", "value": {value}, "test": {test}}}"#)
    };
    let bounds = |min: &str, max: &str| format!(r#"{{"min": {min}, "max": {max}}}"#);
    let cases = [
        (int("3", r#"{"min": 1, "max": 5}"#), true, 0),
        (int("3", "3"), true, 0),
        (int("3", "4"), false, 0),
        (int("3", r#"{"min": 4}"#), false, 0),
        (int("3", r#"{"max": 2}"#), false, 0),
        (int(INT_DRAW, "{}"), true, 0),
        (int(INT_DRAW, r#"{"min": 0}"#), true, 1),
        (int("3", &bounds("4", INT_DRAW)), false, 0),
        (int("3", &bounds("1", INT_DRAW)), false, 1),
        (int("3", &bounds(INT_DRAW, "5")), true, 1),
        (float("0.5", r#"{"min": 0.25, "max": 0.75}"#), true, 0),
        (float("0.5", r#"{"max": 0.25}"#), false, 0),
        (float("0.5", "0.5"), true, 0),
        (float(FLOAT_DRAW, "{}"), true, 0),
        (float(FLOAT_DRAW, r#"{"max": 1.0}"#), true, 1),
        (float("-1.0", &bounds(FLOAT_DRAW, FLOAT_DRAW)), false, 1),
        (float("2.0", &bounds(FLOAT_DRAW, FLOAT_DRAW)), false, 2),
    ];
    for (json, expected, draws) in cases {
        let result = mob_test(&condition(&json), &mob_context(), &Mobs::default());
        assert_eq!(result, (expected, after_floats(draws)), "{json}");
    }
}

#[test]
fn loot_cnd_019_environment_attribute_check_reads_by_position() {
    let check = |attribute: &str, value: &str| {
        format!(
            r#"{{"type": "minecraft:environment_attribute_check", "attribute": "minecraft:{attribute}", "value": {value}}}"#
        )
    };
    let fog = |value: &str| check("visual/fog_end_distance", value);
    let fog_start = |value: &str| check("visual/fog_start_distance", value);
    let fast_lava = |value: &str| check("gameplay/fast_lava", value);
    let at = Some(DVec3::new(0.5, 64.5, 0.5));
    let at_origin = [(at, AttributeValue::Float(8.0))];
    let in_dimension = [(None, AttributeValue::Float(8.0))];
    let negative_zero = [(at, AttributeValue::Float(-0.0))];
    let zero = [(at, AttributeValue::Float(0.0))];
    let lava_in_dimension = [(None, AttributeValue::Boolean(true))];
    let ash = |probability: f32| {
        let particle = NbtCompound::from_iter([("type".to_owned(), "minecraft:ash".into())]);
        let entry = NbtCompound::from_iter([
            ("particle".to_owned(), particle.into()),
            ("probability".to_owned(), probability.into()),
        ]);
        let list = NbtTag::List(vec![entry.into()]);
        AttributeValue::read(AttributeType::AmbientParticles, &list).expect("an ambient particle")
    };
    let ash_negative_zero = [(at, ash(-0.0))];
    let particles = check(
        "visual/ambient_particles",
        r#"[{"particle": {"type": "minecraft:ash"}, "probability": 0.0}]"#,
    );
    let bed_rule = check(
        "gameplay/bed_rule",
        r#"{"can_sleep": "when_dark", "can_set_spawn": "always", "error_message": {"translate": "block.minecraft.bed.no_sleep"}}"#,
    );
    let cases: [(String, &[_], _, _); 13] = [
        (fog("8.0"), &at_origin, at, true),
        (fog("8.0"), &at_origin, None, false),
        (fog("1024.0"), &at_origin, None, true),
        (fog("8.0"), &in_dimension, at, false),
        (fog_start("0.0"), &negative_zero, at, false),
        (fog_start("-0.0"), &negative_zero, at, true),
        (fog_start("-0.0"), &zero, at, false),
        (particles, &ash_negative_zero, at, false),
        (bed_rule.clone(), &[], None, true),
        (bed_rule.replace("when_dark", "never"), &[], None, false),
        (fast_lava("true"), &lava_in_dimension, at, true),
        (fast_lava("true"), &[], at, false),
        (fast_lava("false"), &[], None, true),
    ];
    for (json, environment, origin, expected) in cases {
        let mut context = mob_context();
        context.origin = origin;
        let host = Mobs {
            environment,
            ..Mobs::default()
        };
        let result = mob_test(&condition(&json), &context, &host);
        assert_eq!(
            result,
            (expected, fresh()),
            "{json} {environment:?} {origin:?}"
        );
    }
}
