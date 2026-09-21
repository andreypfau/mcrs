use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::decode_component_value;
use mcrs_minecraft_protocol::item::{
    AdditionalTradeCost, CreativeSlotLock, ItemComponentKind, ItemComponentValue,
    MapPostProcessing, MinimumAttackCharge, PotionDurationScale, hash_ops,
};
use serde::Deserialize;

use crate::harness::{
    TestLookup, decode, from_json, hex, json_value, nbt_tree, persistent_json, wire,
};

#[derive(Deserialize)]
struct Golden {
    values: Vec<Value>,
    errors: Vec<Failure>,
    decodes: Vec<Decoded>,
    decode_errors: Vec<DecodeFailure>,
}

#[derive(Deserialize)]
struct Value {
    kind: String,
    input: Option<String>,
    value: Option<String>,
    json: Option<String>,
    nbt: Option<String>,
    hash: Option<i32>,
    wire: String,
}

#[derive(Deserialize)]
struct Failure {
    kind: String,
    input: String,
    error: String,
}

#[derive(Deserialize)]
struct Decoded {
    kind: String,
    wire: String,
    json: String,
}

#[derive(Deserialize)]
struct DecodeFailure {
    kind: String,
    wire: String,
    error: String,
}

fn golden() -> Golden {
    serde_json::from_str(include_str!("../fixtures/item/plain_golden.json")).unwrap()
}

fn kind(id: &str) -> ItemComponentKind {
    ItemComponentKind::from_id(id).unwrap_or_else(|| panic!("{id} is not a kind"))
}

fn transient_value(kind: ItemComponentKind, text: &str) -> ItemComponentValue {
    match (kind, text) {
        (ItemComponentKind::AdditionalTradeCost, n) => {
            AdditionalTradeCost(n.parse().unwrap()).into()
        }
        (ItemComponentKind::CreativeSlotLock, _) => CreativeSlotLock.into(),
        (ItemComponentKind::MapPostProcessing, "LOCK") => MapPostProcessing::Lock.into(),
        (ItemComponentKind::MapPostProcessing, "SCALE") => MapPostProcessing::Scale.into(),
        _ => panic!("no transient value for {kind} = {text}"),
    }
}

#[test]
fn persistent_values_match_vanilla_in_json_nbt_hash_and_wire() {
    let lookup = TestLookup::new();
    let mut checked = 0;
    for row in golden().values {
        let Some(input) = &row.input else { continue };
        let kind = kind(&row.kind);
        let value = from_json(kind, input);
        let json = persistent_json(&value);
        assert_eq!(
            json_value(&value),
            serde_json::from_str::<serde_json::Value>(row.json.as_ref().unwrap()).unwrap(),
            "{kind} json of {input}"
        );
        assert_eq!(from_json(kind, &json), value, "{kind} rereads {json}");

        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&value, &mut nbt).unwrap();
        assert_eq!(
            nbt_tree(&nbt),
            nbt_tree(&hex(row.nbt.as_ref().unwrap())),
            "{kind} nbt of {input}"
        );
        assert_eq!(
            hash_ops::hash(&value).unwrap(),
            row.hash.unwrap(),
            "{kind} hash of {input}"
        );

        let bytes = hex(&row.wire);
        assert_eq!(wire(&lookup, &value), bytes, "{kind} wire of {input}");
        assert_eq!(
            decode(&lookup, kind, &bytes),
            value,
            "{kind} wire decode of {input}"
        );
        checked += 1;
    }
    assert!(checked > 100, "{checked} rows checked");
}

#[test]
fn transient_values_match_the_vanilla_wire() {
    let lookup = TestLookup::new();
    let mut checked = 0;
    for row in golden().values {
        let Some(text) = &row.value else { continue };
        let kind = kind(&row.kind);
        assert!(!kind.is_persistent(), "{kind}");
        let value = transient_value(kind, text);
        let bytes = hex(&row.wire);
        assert_eq!(wire(&lookup, &value), bytes, "{kind} wire of {text}");
        assert_eq!(
            decode(&lookup, kind, &bytes),
            value,
            "{kind} wire decode of {text}"
        );
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn rejected_inputs_are_rejected_with_the_vanilla_range_messages() {
    let mut checked = 0;
    for row in golden().errors {
        let kind = kind(&row.kind);
        let mut d = serde_json::Deserializer::from_str(&row.input);
        let error = ItemComponentValue::deserialize_value(kind, &mut d)
            .err()
            .unwrap_or_else(|| panic!("{kind} accepted {}", row.input));
        if row.error.starts_with("Value must") {
            let message = error.to_string();
            assert!(
                message.starts_with(&row.error),
                "{kind} on {}: {error} is not {}",
                row.input,
                row.error
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 49);
}

#[test]
fn records_read_a_list_no_better_than_vanilla_does() {
    for (kind, tag) in [
        (
            ItemComponentKind::Enchantable,
            NbtTag::List(vec![NbtTag::Int(7)]),
        ),
        (ItemComponentKind::Enchantable, NbtTag::Int(7)),
        (
            ItemComponentKind::VillagerFood,
            NbtTag::List(vec![NbtTag::Int(7)]),
        ),
        (ItemComponentKind::AttackAnimation, NbtTag::List(vec![])),
        (
            ItemComponentKind::AttackAnimation,
            NbtTag::List(vec![NbtTag::String("stab".into()), NbtTag::Int(10)]),
        ),
        (ItemComponentKind::InteractAnimation, NbtTag::Int(6)),
    ] {
        assert!(
            ItemComponentValue::deserialize_value(kind, tag.clone()).is_err(),
            "{kind} read {tag:?}"
        );
    }
}

#[test]
fn wire_values_the_vanilla_constructor_refuses_fail_to_decode() {
    let mut checked = 0;
    for row in golden().decode_errors {
        let kind = kind(&row.kind);
        let bytes = hex(&row.wire);
        let error = decode_component_value(kind, &TestLookup::new(), &mut &bytes[..])
            .err()
            .unwrap_or_else(|| panic!("{kind} decoded {}", row.wire));
        assert!(
            format!("{error:#}").contains(&row.error),
            "{kind} on {}: {error:#} is not {}",
            row.wire,
            row.error
        );
        checked += 1;
    }
    assert_eq!(checked, 2);
}

#[test]
fn out_of_range_floats_are_refused_on_write_as_well() {
    fn refused(value: &(impl serde::Serialize + std::fmt::Debug), message: &str) {
        let json = serde_json::to_string(value).unwrap_err().to_string();
        assert_eq!(json, message, "{value:?} to json");
        let mut nbt = Vec::new();
        let error = mcrs_minecraft_nbt::to_bytes_unnamed(value, &mut nbt).unwrap_err();
        assert!(
            error.to_string().contains(message),
            "{value:?} to nbt: {error}"
        );
    }
    refused(
        &MinimumAttackCharge(-0.0),
        "Value must be within range [0.0;1.0]: -0.0",
    );
    refused(
        &MinimumAttackCharge(1.5),
        "Value must be within range [0.0;1.0]: 1.5",
    );
    refused(
        &MinimumAttackCharge(f32::NAN),
        "Value must be within range [0.0;1.0]: NaN",
    );
    refused(
        &PotionDurationScale(-1.0),
        "Value must be non-negative: -1.0",
    );
    assert_eq!(
        serde_json::to_string(&MinimumAttackCharge(1.0)).unwrap(),
        "1.0"
    );
}

#[test]
fn out_of_range_wire_ids_decode_as_vanilla_does() {
    let lookup = TestLookup::new();
    let mut checked = 0;
    for row in golden().decodes {
        let kind = kind(&row.kind);
        let value = decode(&lookup, kind, &hex(&row.wire));
        if kind.is_persistent() {
            assert_eq!(
                json_value(&value),
                serde_json::from_str::<serde_json::Value>(&row.json).unwrap(),
                "{kind} from {}",
                row.wire
            );
        } else {
            assert_eq!(
                value,
                transient_value(kind, &row.json),
                "{kind} from {}",
                row.wire
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 32);
}
