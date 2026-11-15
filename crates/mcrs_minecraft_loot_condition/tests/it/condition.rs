use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, Damage, ItemStack, Kind};
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    Condition, ConditionBaker, ConditionFile, ContextParam, LootContext, ParamNotAllowed, Tool,
};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_voxel::BlockStateId;

use crate::number_provider::{VANILLA, inputs};

/// The condition of `json`, baked against the vanilla entries without a problem.
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
        inputs(&VANILLA, context),
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
    for (json, key_set, refused) in cases {
        let reads = condition(json).reads();
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
    let context = empty_context();
    for (rule, json, passes, after) in cases {
        let mut random = fresh();
        let result = condition(json).test(
            inputs(&VANILLA, &context),
            &mut random,
            &mut PredicateScratch::default(),
        );
        assert_eq!(result, passes, "{rule}: {json}");
        assert_eq!(random, after, "{rule}: the draws of {json}");
    }
}

/// REG-FLD-006, REG-ENTRY-003: an unknown condition type fails the decode, a condition and its
/// block, item and location predicates ignore a field that they do not name, and an entity
/// predicate fails on an unknown top-level key.
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

/// REG-FLD-008, REG-FLD-254, REG-FLD-719: the `value` of `environment_attribute_check` decodes in
/// the JSON form of the attribute type. REG-FLD-720: an object with `type` in a range is a provider,
/// and an unknown provider type fails the decode.
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

/// LOOT-CND-006: `match_block` fails without `block_state`, and matches the block entity data
/// when its predicate has `nbt`.
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

/// LOOT-CND-007: `match_tool` matches the components of the tool, and fails for an empty hand
/// that its item predicate does not match.
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

/// LOOT-CND-009, LOOT-CND-020: `entity_properties` tests the entity of its `entity` parameter;
/// the entity predicate `{}` matches each present entity and no absent one.
#[test]
fn loot_cnd_009_entity_properties_tests_its_target() {
    let any_this = r#"{"type": "minecraft:entity_properties", "entity": "this", "predicate": {}}"#;
    let any_attacker =
        r#"{"type": "minecraft:entity_properties", "entity": "attacker", "predicate": {}}"#;
    let pig = r#"{"type": "minecraft:entity_properties", "entity": "this", "predicate": {"entity_type": "minecraft:pig"}}"#;
    let cases = [
        (any_this, Some(()), None, true),
        (any_this, None, Some(()), false),
        (any_attacker, None, Some(()), true),
        (any_attacker, Some(()), None, false),
        (pig, Some(()), None, false),
    ];
    for (json, this, attacker, expected) in cases {
        let mut context = empty_context();
        context.this_entity = this;
        context.attacking_entity = attacker;
        assert_eq!(
            passes(&condition(json), &context),
            expected,
            "{json} {this:?} {attacker:?}"
        );
    }
}
