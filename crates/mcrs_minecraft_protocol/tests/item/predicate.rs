use mcrs_minecraft_protocol::item::decode_component_value;
use mcrs_minecraft_protocol::item::{
    ComponentPredicateType, ItemComponentKind, ItemComponentValue, hash_ops,
};
use mcrs_minecraft_registry::{RegistryLookup, StaticRegistryTable};
use serde::Deserialize;

use crate::harness::{PersistentValue, TestLookup, decode, from_json, hex, json_value, wire};

#[derive(Deserialize)]
struct Case {
    kind: String,
    input: String,
    json: serde_json::Value,
    hash: i32,
    wire: String,
    /// Vanilla emits `state` and NBT compounds in hash order, so an unordered
    /// case compares through the decoded value and the order-independent hash
    /// instead of its bytes.
    ordered: bool,
}

fn lookup() -> TestLookup {
    let mut lookup = TestLookup::new();
    lookup.registry_with_ids("block", &[("stone", 1), ("dirt", 9)]);
    lookup
}

#[test]
fn predicates_match_the_vanilla_codecs() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/item/predicate_vanilla.json")).unwrap();
    assert_eq!(cases.len(), 81);
    let lookup = lookup();
    for case in cases {
        let kind = ItemComponentKind::from_id(&case.kind).unwrap();
        let label = format!("{} {}", case.kind, case.input);
        let value = from_json(kind, &case.input);
        assert_eq!(json_value(&value), case.json, "{label}");
        assert_eq!(
            hash_ops::hash(&PersistentValue(&value)).unwrap(),
            case.hash,
            "{label}"
        );

        let bytes = hex(&case.wire);
        if case.ordered {
            assert_eq!(wire(&lookup, &value), bytes, "{label}");
        }
        let decoded = decode(&lookup, kind, &bytes);
        assert_eq!(json_value(&decoded), case.json, "{label} from the wire");
        assert_eq!(
            hash_ops::hash(&PersistentValue(&decoded)).unwrap(),
            case.hash,
            "{label} from the wire"
        );
        if case.ordered {
            assert_eq!(wire(&lookup, &decoded), bytes, "{label} re-encoded");
        } else {
            assert_eq!(
                decode(&lookup, kind, &wire(&lookup, &decoded)),
                decoded,
                "{label} re-encoded"
            );
        }
    }
}

#[test]
fn predicate_errors_read_like_vanilla() {
    let error = |kind: ItemComponentKind, json: &str| -> String {
        let mut d = serde_json::Deserializer::from_str(json);
        ItemComponentValue::deserialize_value(kind, &mut d)
            .err()
            .unwrap_or_else(|| panic!("{json} was accepted"))
            .to_string()
    };
    for (json, expected) in [
        ("[]", "List must have contents"),
        (
            "[{\"nbt\":\"not a compound\"}]",
            "SNBT syntax error at 4: trailing data",
        ),
        (
            "{\"components\":{\"minecraft:creative_slot_lock\":{}}}",
            "'minecraft:creative_slot_lock' is not a persistent component",
        ),
        (
            "{\"predicates\":{\"minecraft:nope\":{}}}",
            "Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_predicate_type]: minecraft:nope",
        ),
        (
            "{\"predicates\":{\"minecraft:max_stack_size\":\"junk\"}}",
            "invalid type: string \"junk\", expected a map",
        ),
        ("{\"foo\":1}", "unknown field `foo`"),
        (
            "{\"state\":{\"lit\":\"true\",\"lit\":\"false\"}}",
            "Duplicate key 'lit'",
        ),
        (
            "{\"blocks\":\"a\",\"blocks\":\"b\"}",
            "duplicate field `blocks`",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":\"junk\"}}",
            "invalid type: string \"junk\", expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":[1,2]}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":true}}",
            "invalid type: boolean `true`, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":{\"durability\":{\"min\":5,\"max\":2}}}}",
            "Swapped bounds in range: Optional[5] is higher than Optional[2]",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":{\"durability\":{\"min\":\"x\"}}}}",
            "invalid type: string \"x\", expected a number",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":{\"foo\":1}}}",
            "unknown field `foo`, expected `durability` or `damage`",
        ),
        (
            "{\"predicates\":{\"minecraft:enchantments\":{}}}",
            "invalid type: map, expected a sequence",
        ),
        (
            "{\"predicates\":{\"minecraft:firework_explosion\":{\"shape\":\"nope\"}}}",
            "unknown variant `nope`",
        ),
        (
            "{\"predicates\":{\"minecraft:villager/variant\":{}}}",
            "invalid type: map, expected",
        ),
        (
            "{\"predicates\":{\"minecraft:attribute_modifiers\":{\"modifiers\":{\"contains\":[{\"amount\":{\"min\":3,\"max\":1}}]}}}}",
            "Swapped bounds in range: Optional[3.0] is higher than Optional[1.0]",
        ),
        (
            "{\"predicates\":{\"minecraft:attribute_modifiers\":{\"modifiers\":{\"contains\":[{\"amount\":{\"min\":0.0,\"max\":-0.0}}]}}}}",
            "Swapped bounds in range: Optional[0.0] is higher than Optional[-0.0]",
        ),
        (
            "{\"predicates\":{\"minecraft:attribute_modifiers\":{\"modifiers\":{\"contains\":[{\"amount\":{\"min\":1e300,\"max\":1e-300}}]}}}}",
            "Swapped bounds in range: Optional[1.0E300] is higher than Optional[1.0E-300]",
        ),
        (
            "{\"predicates\":{\"minecraft:attribute_modifiers\":{\"modifiers\":{\"contains\":[[]]}}}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":{\"durability\":{\"min\":5.1,\"max\":4.9}}}}",
            "Swapped bounds in range: Optional[5] is higher than Optional[4]",
        ),
        (
            "{\"predicates\":{\"minecraft:firework_explosion\":{\"has_twinkle\":0,\"has_trail\":1}}}",
            "invalid type: integer `0`, expected a boolean",
        ),
        (
            "{\"predicates\":{\"minecraft:potion_contents\":{\"effects\":{\"contains\":[{\"minecraft:speed\":[1,2]}]}}}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:potion_contents\":{\"effects\":{\"contains\":[{\"minecraft:speed\":{\"ambient\":1}}]}}}}",
            "invalid type: integer `1`, expected a boolean",
        ),
        (
            "{\"predicates\":{\"minecraft:potion_contents\":{\"effects\":{\"count\":[[\"a\",1]]}}}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:fireworks\":{\"explosions\":{\"contains\":[[]]}}}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:fireworks\":{\"explosions\":[]}}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:enchantments\":[[]]}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:trim\":[]}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"predicates\":{\"minecraft:potion_contents\":{\"effects\":{\"contains\":[{\"minecraft:speed\":{},\"minecraft:speed\":{}}]}}}}",
            "Duplicate key 'minecraft:speed'",
        ),
    ] {
        let message = error(ItemComponentKind::CanBreak, json);
        assert!(message.starts_with(expected), "{json}: {message}");
    }
    for (json, expected) in [
        (
            "{\"predicates\":{\"minecraft:damage\":{},\"damage\":{}}}",
            "Duplicate key 'damage'",
        ),
        (
            "{\"count\":{\"min\":5,\"max\":2}}",
            "Swapped bounds in range: Optional[5] is higher than Optional[2]",
        ),
        (
            "{\"predicates\":{\"minecraft:damage\":[1,2]}}",
            "invalid type: sequence, expected a map",
        ),
        (
            "{\"count\":true}",
            "invalid type: boolean `true`, expected a number",
        ),
    ] {
        let message = error(ItemComponentKind::Lock, json);
        assert!(message.starts_with(expected), "{json}: {message}");
    }
}

/// A predicate the wire carries in a shape its codec refuses fails to decode
/// instead of passing through.
#[test]
fn malformed_predicate_values_are_refused_on_the_wire() {
    for (wire, expected) in [
        (
            "01000000000101000800046a756e6b",
            "Trying to deserialize a map without a compound ID (id 8)",
        ),
        (
            "01000000000101000a0a000a6475726162696c6974790300036d696e000000050300036d6178000000020000",
            "Swapped bounds in range",
        ),
        ("010000000001010e0a00", "invalid type: map, expected"),
    ] {
        let bytes = hex(wire);
        let mut r = &bytes[..];
        let error =
            decode_component_value(ItemComponentKind::CanBreak, &lookup(), &mut r).unwrap_err();
        assert!(error.to_string().contains(expected), "{wire}: {error}");
    }
}

#[test]
fn the_partial_predicate_list_is_capped_on_the_wire() {
    let mut wire = vec![0x01, 0x00, 0x00, 0x00, 0x00, 65];
    wire.extend(std::iter::repeat_n([0x00, 0x01, 0x0A, 0x00], 65).flatten());
    let mut r = &wire[..];
    let error = decode_component_value(ItemComponentKind::CanBreak, &lookup(), &mut r).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("list of 65 entries exceeds the maximum of 64"),
        "{error}"
    );
}

/// `-0.0` and `0.0` are distinct bounds, so this range is not a point. It
/// stays out of the golden table because vanilla's own wire folds every zero
/// into one tag and loses the sign.
#[test]
fn a_double_range_between_the_two_zeros_is_kept() {
    let input = "{\"predicates\":{\"minecraft:attribute_modifiers\":{\"modifiers\":{\"contains\":[{\"amount\":{\"min\":-0.0,\"max\":0.0}}]}}}}";
    let value = from_json(ItemComponentKind::CanBreak, input);
    assert_eq!(
        json_value(&value),
        serde_json::from_str::<serde_json::Value>(input).unwrap()
    );
    assert_eq!(hash_ops::hash(&PersistentValue(&value)).unwrap(), 816994624);
}

/// Partial predicates are a map, so two predicate lists that differ only in
/// order are the same value; the NBT predicate compares by key rather than
/// by position.
#[test]
fn predicate_order_does_not_affect_equality() {
    let lookup = lookup();
    for (a, b) in [
        (
            "{\"predicates\":{\"minecraft:damage\":{},\"minecraft:trim\":{}}}",
            "{\"predicates\":{\"minecraft:trim\":{},\"minecraft:damage\":{}}}",
        ),
        (
            "{\"predicates\":{\"minecraft:custom_data\":{\"b\":1,\"a\":2}}}",
            "{\"predicates\":{\"minecraft:custom_data\":{\"a\":2,\"b\":1}}}",
        ),
        (
            "{\"nbt\":{\"b\":1,\"a\":{\"y\":2,\"x\":1}}}",
            "{\"nbt\":{\"a\":{\"x\":1,\"y\":2},\"b\":1}}",
        ),
    ] {
        let a = from_json(ItemComponentKind::CanBreak, a);
        let b = from_json(ItemComponentKind::CanBreak, b);
        assert_eq!(a, b);
        assert_eq!(
            decode(&lookup, ItemComponentKind::CanBreak, &wire(&lookup, &a)),
            a
        );
    }
    let a = from_json(
        ItemComponentKind::CanBreak,
        "{\"predicates\":{\"minecraft:damage\":{},\"minecraft:trim\":{}}}",
    );
    let b = from_json(
        ItemComponentKind::CanBreak,
        "{\"predicates\":{\"minecraft:damage\":{\"damage\":1},\"minecraft:trim\":{}}}",
    );
    assert_ne!(a, b);
}

#[test]
fn predicate_type_ids_are_the_registry_protocol_ids() {
    let report = StaticRegistryTable::from_json(include_bytes!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let entries = report.registry("data_component_predicate_type").unwrap();
    assert_eq!(entries.len(), ComponentPredicateType::ALL.len());
    for kind in ComponentPredicateType::ALL {
        assert_eq!(
            report.id("data_component_predicate_type", &kind.id().into()),
            Some(*kind as u32),
            "{kind:?}"
        );
        assert_eq!(
            ComponentPredicateType::from_id(kind.id().as_str()),
            Some(*kind)
        );
        assert_eq!(
            ComponentPredicateType::from_id(kind.id().path()),
            Some(*kind)
        );
    }
}
