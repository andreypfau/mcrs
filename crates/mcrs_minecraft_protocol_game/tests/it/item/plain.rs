use crate::item::harness::{decode_component, wire_kind};
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::ErrorKind;
use mcrs_minecraft_protocol_game::item::{
    AdditionalTradeCost, CreativeSlotLock, ItemComponentValue, MapPostProcessing,
    MinimumAttackCharge, PotionDurationScale, hash_ops,
};
use serde::Deserialize;
use std::io::Cursor;

use crate::common::sorted;

use crate::item::harness::{
    WireCodec, decode, from_json, hex, json_value, lookup, named, nbt_tree, persistent_json,
    try_from_json, wire,
};

struct Deviating {
    deviation: &'static str,
    kind: &'static str,
    input: &'static str,
    json: &'static str,
}

const DEVIATING: [Deviating; 1] = [Deviating {
    deviation: "D13",
    kind: "container_loot",
    input: r#"{"loot_table":"minecraft:chests/x","seed":18446744073709551617}"#,
    json: r#"{"loot_table":"minecraft:chests/x","seed":9223372036854775807}"#,
}];

const REFUSED_ON_THE_WIRE: [(&str, &str); 1] = [(
    "lodestone_tracker",
    r#"{"target":{"dimension":"mcrs:custom","pos":[0,4096,-1]}}"#,
)];

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
    serde_json::from_str(&fixture("item_plain").text()).unwrap()
}

fn kind(id: &str) -> DataComponentType {
    DataComponentType::read(id).unwrap_or_else(|| panic!("{id} is not a kind"))
}

fn whole_nbt_tree(bytes: &[u8]) -> NbtTag {
    let mut cursor = Cursor::new(bytes);
    let tree = mcrs_minecraft_nbt::from_bytes_unnamed(&mut cursor).unwrap();
    assert_eq!(
        cursor.position(),
        u64::try_from(bytes.len()).unwrap(),
        "the component is one NBT value with no trailing bytes"
    );
    sorted(tree)
}

fn wire_matches(kind: DataComponentType, ours: &[u8], vanilla: &[u8]) -> bool {
    if kind == DataComponentType::MapDecorations {
        whole_nbt_tree(ours) == whole_nbt_tree(vanilla)
    } else {
        ours == vanilla
    }
}

#[derive(Debug, PartialEq)]
struct TooLong {
    len: usize,
    max: usize,
}

fn too_long(message: &str) -> Option<TooLong> {
    let (len, max) = message.split_once(" elements exceeded max size of: ")?;
    Some(TooLong {
        len: len.parse().ok()?,
        max: max.parse().ok()?,
    })
}

fn unknown_id(message: &str) -> Option<String> {
    let id: u32 = message.strip_prefix("No value with id ")?.parse().ok()?;
    Some(format!("has no id {id}"))
}

fn transient_value(kind: DataComponentType, text: &str) -> ItemComponentValue {
    match (kind, text) {
        (DataComponentType::AdditionalTradeCost, n) => {
            AdditionalTradeCost(n.parse().unwrap()).into()
        }
        (DataComponentType::CreativeSlotLock, _) => CreativeSlotLock.into(),
        (DataComponentType::MapPostProcessing, "LOCK") => MapPostProcessing::Lock.into(),
        (DataComponentType::MapPostProcessing, "SCALE") => MapPostProcessing::Scale.into(),
        _ => panic!("no transient value for {kind:?} = {text}"),
    }
}

struct SeenProfile {
    value: ItemComponentValue,
    wire: String,
}

fn assert_saved_form_and_hash(
    kind: DataComponentType,
    value: &ItemComponentValue,
    row: &Value,
    input: &str,
) {
    let json = persistent_json(value);
    assert_eq!(
        json_value(value),
        serde_json::from_str::<serde_json::Value>(row.json.as_ref().unwrap()).unwrap(),
        "{kind:?} json of {input}"
    );
    let reread = from_json(kind, &json);
    if kind == DataComponentType::CustomData {
        let again = persistent_json(&reread);
        assert_eq!(from_json(kind, &again), reread, "{kind:?} rereads {again}");
    } else {
        assert_eq!(&reread, value, "{kind:?} rereads {json}");
    }

    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&named(value), &mut nbt).unwrap();
    assert_eq!(
        nbt_tree(&nbt),
        nbt_tree(&hex(row.nbt.as_ref().unwrap())),
        "{kind:?} nbt of {input}"
    );
    assert_eq!(
        hash_ops::hash(&named(value)).unwrap(),
        row.hash.unwrap(),
        "{kind:?} hash of {input}"
    );
}

#[test]
fn persistent_values_match_vanilla_in_json_nbt_hash_and_wire() {
    let lookup = lookup();
    let mut checked = 0;
    let mut profiles: Vec<SeenProfile> = Vec::new();
    for row in golden().values {
        let Some(input) = &row.input else { continue };
        let kind = kind(&row.kind);
        let value = from_json(kind, input);
        if let Some(case) = DEVIATING
            .iter()
            .find(|case| case.kind == row.kind && case.input == input)
        {
            assert_eq!(
                json_value(&value),
                serde_json::from_str::<serde_json::Value>(case.json).unwrap(),
                "{}: {kind:?} json of {input}",
                case.deviation
            );
            checked += 1;
            continue;
        }
        let cached_by_the_game = kind == DataComponentType::Profile
            && profiles
                .iter()
                .any(|seen| seen.value == value && seen.wire != row.wire);
        if kind == DataComponentType::Profile {
            profiles.push(SeenProfile {
                value: value.clone(),
                wire: row.wire.clone(),
            });
        }
        if !cached_by_the_game {
            assert_saved_form_and_hash(kind, &value, &row, input);
        }

        let bytes = hex(&row.wire);
        if REFUSED_ON_THE_WIRE.contains(&(row.kind.as_str(), input.as_str())) {
            assert!(
                value.encode_checked(&lookup, &mut Vec::new()).is_err(),
                "{kind:?} sent {input}"
            );
            assert_ne!(decode(&lookup, kind, &bytes), value, "{kind:?} of {input}");
            checked += 1;
            continue;
        }
        let ours = wire(&lookup, &value);
        assert!(
            wire_matches(kind, &ours, &bytes),
            "{kind:?} wire of {input}: {ours:02x?} is not {bytes:02x?}"
        );
        assert_eq!(
            decode(&lookup, kind, &bytes),
            value,
            "{kind:?} wire decode of {input}"
        );
        checked += 1;
    }
    assert!(checked > 100, "{checked} rows checked");
}

#[test]
fn transient_values_match_the_vanilla_wire() {
    let lookup = lookup();
    let mut checked = 0;
    for row in golden().values {
        let Some(text) = &row.value else { continue };
        let kind = kind(&row.kind);
        assert!(!kind.is_persistent(), "{kind:?}");
        let value = transient_value(kind, text);
        let bytes = hex(&row.wire);
        assert_eq!(wire(&lookup, &value), bytes, "{kind:?} wire of {text}");
        assert_eq!(
            decode(&lookup, kind, &bytes),
            value,
            "{kind:?} wire decode of {text}"
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
        let message = try_from_json(kind, &row.input)
            .err()
            .unwrap_or_else(|| panic!("{kind:?} accepted {}", row.input));
        if row.error.starts_with("Value must") {
            assert!(
                message.starts_with(&row.error),
                "{kind:?} on {}: {message} is not {}",
                row.input,
                row.error
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 118);
}

#[test]
fn records_read_a_list_no_better_than_vanilla_does() {
    for (kind, tag) in [
        (
            DataComponentType::Enchantable,
            NbtTag::List(vec![NbtTag::Int(7)]),
        ),
        (DataComponentType::Enchantable, NbtTag::Int(7)),
        (
            DataComponentType::VillagerFood,
            NbtTag::List(vec![NbtTag::Int(7)]),
        ),
        (DataComponentType::AttackAnimation, NbtTag::List(vec![])),
        (
            DataComponentType::AttackAnimation,
            NbtTag::List(vec![NbtTag::String("stab".into()), NbtTag::Int(10)]),
        ),
        (DataComponentType::InteractAnimation, NbtTag::Int(6)),
    ] {
        assert!(
            ItemComponentValue::deserialize_value(kind, tag.clone()).is_err(),
            "{kind:?} read {tag:?}"
        );
    }
}

#[test]
fn wire_values_the_vanilla_constructor_refuses_fail_to_decode() {
    let mut checked = 0;
    for row in golden().decode_errors {
        let kind = kind(&row.kind);
        let bytes = hex(&row.wire);
        let error = decode_component(kind, &lookup(), &mut &bytes[..])
            .err()
            .unwrap_or_else(|| panic!("{kind:?} decoded {}", row.wire));
        match too_long(&row.error) {
            Some(expected) => assert!(
                matches!(
                    wire_kind(&error),
                    ErrorKind::TooLong { len, max } if TooLong { len: *len, max: *max } == expected
                ),
                "{kind:?} on {}: {error:#} is not {expected:?}",
                row.wire
            ),
            None => {
                let expected = unknown_id(&row.error).unwrap_or_else(|| row.error.clone());
                assert!(
                    format!("{error:#}").contains(&expected),
                    "{kind:?} on {}: {error:#} is not {}",
                    row.wire,
                    row.error
                );
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 10);
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
    let lookup = lookup();
    let mut checked = 0;
    for row in golden().decodes {
        let kind = kind(&row.kind);
        let value = decode(&lookup, kind, &hex(&row.wire));
        if kind.is_persistent() {
            assert_eq!(
                json_value(&value),
                serde_json::from_str::<serde_json::Value>(&row.json).unwrap(),
                "{kind:?} from {}",
                row.wire
            );
        } else {
            assert_eq!(
                value,
                transient_value(kind, &row.json),
                "{kind:?} from {}",
                row.wire
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 40);
}
