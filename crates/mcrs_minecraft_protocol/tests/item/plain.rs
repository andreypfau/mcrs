//! Values produced by the vanilla 26.3-snapshot-10 codecs of the scalar, unit
//! and enum kinds through `JsonOps`, `NbtOps`, `HashOps.CRC32C_INSTANCE` and
//! each kind's `STREAM_CODEC`.

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::{
    AdditionalTradeCost, CreativeSlotLock, ItemComponentKind, ItemComponentValue,
    MapPostProcessing, hash_ops,
};
use serde::Deserialize;

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

#[derive(Deserialize)]
struct Golden {
    values: Vec<Value>,
    errors: Vec<Failure>,
    decodes: Vec<Decoded>,
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

fn golden() -> Golden {
    serde_json::from_str(include_str!("../fixtures/item/plain_golden.json")).unwrap()
}

fn kind(id: &str) -> ItemComponentKind {
    ItemComponentKind::from_id(id).unwrap_or_else(|| panic!("{id} is not a kind"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn wire(value: &ItemComponentValue) -> Vec<u8> {
    let mut out = Vec::new();
    value
        .encode_ctx_value(&TestLookup::new(), &mut out)
        .unwrap();
    out
}

fn decode(kind: ItemComponentKind, bytes: &[u8]) -> ItemComponentValue {
    let mut r = bytes;
    let value = ItemComponentValue::decode_ctx_value(kind, &TestLookup::new(), &mut r).unwrap();
    assert!(r.is_empty(), "{kind}: {} trailing bytes", r.len());
    value
}

/// Vanilla writes compound keys in hash order, so trees compare sorted.
fn nbt_tree(bytes: &[u8]) -> NbtTag {
    fn sort(tag: &mut NbtTag) {
        if let NbtTag::Compound(compound) = tag {
            compound.child_tags.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, child) in &mut compound.child_tags {
                sort(child);
            }
        }
    }
    let mut tag = mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).unwrap();
    sort(&mut tag);
    tag
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
    let mut checked = 0;
    for row in golden().values {
        let Some(input) = &row.input else { continue };
        let kind = kind(&row.kind);
        let value = from_json(kind, input);
        let json = persistent_json(&value);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            serde_json::from_str::<serde_json::Value>(row.json.as_ref().unwrap()).unwrap(),
            "{kind} json of {input}"
        );
        assert_eq!(from_json(kind, &json), value, "{kind} rereads {json}");

        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(&value), &mut nbt).unwrap();
        assert_eq!(
            nbt_tree(&nbt),
            nbt_tree(&hex(row.nbt.as_ref().unwrap())),
            "{kind} nbt of {input}"
        );
        assert_eq!(
            hash_ops::hash(&PersistentValue(&value)).unwrap(),
            row.hash.unwrap(),
            "{kind} hash of {input}"
        );

        let bytes = hex(&row.wire);
        assert_eq!(wire(&value), bytes, "{kind} wire of {input}");
        assert_eq!(decode(kind, &bytes), value, "{kind} wire decode of {input}");
        checked += 1;
    }
    assert!(checked > 100, "{checked} rows checked");
}

#[test]
fn transient_values_match_the_vanilla_wire() {
    let mut checked = 0;
    for row in golden().values {
        let Some(text) = &row.value else { continue };
        let kind = kind(&row.kind);
        assert!(!kind.is_persistent(), "{kind}");
        let value = transient_value(kind, text);
        let bytes = hex(&row.wire);
        assert_eq!(wire(&value), bytes, "{kind} wire of {text}");
        assert_eq!(decode(kind, &bytes), value, "{kind} wire decode of {text}");
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
            // ponytail: `Bounded` in mcrs_minecraft_core words POSITIVE_INT and
            // NON_NEGATIVE_INT as a plain range; give it the vanilla wording and
            // drop the second form here.
            let bounded = row
                .error
                .replace(
                    "Value must be positive: ",
                    "Value must be within range [1;2147483647]: ",
                )
                .replace(
                    "Value must be non-negative: ",
                    "Value must be within range [0;2147483647]: ",
                );
            let message = error.to_string();
            assert!(
                message.starts_with(&row.error) || message.starts_with(&bounded),
                "{kind} on {}: {error} is not {}",
                row.input,
                row.error
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 33);
}

#[test]
fn out_of_range_wire_ids_decode_as_vanilla_does() {
    let mut checked = 0;
    for row in golden().decodes {
        let kind = kind(&row.kind);
        let value = decode(kind, &hex(&row.wire));
        if kind.is_persistent() {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&persistent_json(&value)).unwrap(),
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
