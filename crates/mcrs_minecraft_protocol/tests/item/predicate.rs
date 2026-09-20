//! Values produced by the vanilla 26.3-snapshot-10 codecs of `can_place_on`,
//! `can_break` and `lock`. Vanilla emits `state` and NBT compounds in hash
//! order, so a case marked unordered compares through the decoded value and
//! the order-independent CRC32C hash instead of its bytes.

use std::collections::BTreeMap;

use mcrs_minecraft_protocol::item::{
    ComponentPredicateType, ItemComponentKind, ItemComponentValue, hash_ops,
};
use serde::Deserialize;

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

#[derive(Deserialize)]
struct Case {
    kind: String,
    input: String,
    json: serde_json::Value,
    hash: i32,
    wire: String,
    ordered: bool,
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn lookup() -> TestLookup {
    let mut lookup = TestLookup::new();
    lookup.registry_with_ids("block", &[("stone", 1), ("dirt", 9)]);
    lookup
}

fn json_value(value: &ItemComponentValue) -> serde_json::Value {
    serde_json::from_str(&persistent_json(value)).unwrap()
}

fn wire(value: &ItemComponentValue) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode_ctx_value(&lookup(), &mut out).unwrap();
    out
}

fn decode(kind: ItemComponentKind, bytes: &[u8]) -> ItemComponentValue {
    let mut r = bytes;
    let value = ItemComponentValue::decode_ctx_value(kind, &lookup(), &mut r).unwrap();
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    value
}

#[test]
fn predicates_match_the_vanilla_codecs() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/item/predicate_vanilla.json")).unwrap();
    assert_eq!(cases.len(), 64);
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
            assert_eq!(wire(&value), bytes, "{label}");
        }
        let decoded = decode(kind, &bytes);
        assert_eq!(json_value(&decoded), case.json, "{label} from the wire");
        assert_eq!(
            hash_ops::hash(&PersistentValue(&decoded)).unwrap(),
            case.hash,
            "{label} from the wire"
        );
        if case.ordered {
            assert_eq!(wire(&decoded), bytes, "{label} re-encoded");
        } else {
            assert_eq!(decode(kind, &wire(&decoded)), decoded, "{label} re-encoded");
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
            "invalid type: string \"x\", expected i32",
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
            "Swapped bounds in range: Optional[3] is higher than Optional[1]",
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
    ] {
        let message = error(ItemComponentKind::Lock, json);
        assert!(message.starts_with(expected), "{json}: {message}");
    }
}

/// A predicate the wire carries in a shape its codec refuses fails to decode,
/// as vanilla's `fromCodecWithRegistries` does, instead of passing through.
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
            ItemComponentValue::decode_ctx_value(ItemComponentKind::CanBreak, &lookup(), &mut r)
                .unwrap_err();
        assert!(error.to_string().contains(expected), "{wire}: {error}");
    }
}

#[test]
fn the_partial_predicate_list_is_capped_on_the_wire() {
    let mut wire = vec![0x01, 0x00, 0x00, 0x00, 0x00, 65];
    wire.extend(std::iter::repeat_n([0x00, 0x01, 0x0A, 0x00], 65).flatten());
    let mut r = &wire[..];
    let error =
        ItemComponentValue::decode_ctx_value(ItemComponentKind::CanBreak, &lookup(), &mut r)
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("list of 65 entries exceeds the maximum of 64"),
        "{error}"
    );
}

#[derive(Deserialize)]
struct Report {
    entries: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    protocol_id: u32,
}

#[test]
fn predicate_type_ids_are_the_registry_protocol_ids() {
    let report: BTreeMap<String, Report> = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let entries = &report["minecraft:data_component_predicate_type"].entries;
    assert_eq!(entries.len(), ComponentPredicateType::ALL.len());
    for kind in ComponentPredicateType::ALL {
        assert_eq!(
            entries[kind.id().as_str()].protocol_id,
            *kind as u32,
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
