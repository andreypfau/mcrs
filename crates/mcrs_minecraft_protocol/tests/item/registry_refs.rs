//! Values produced by the vanilla 26.3-snapshot-10 codecs of every kind that
//! references a registry by raw id or holder set, captured with the world
//! registries loaded from the vanilla data pack. Each `sample` block of the
//! fixture holds the JSON vanilla wrote back, the NBT and wire bytes, and the
//! `HashOps.CRC32C_INSTANCE` hash; `id` lines are the registry ids the capture
//! session had.

use std::collections::BTreeMap;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::{ItemComponentKind, ItemComponentValue, hash_ops};

use crate::harness::{PersistentValue, TestLookup, persistent_json};

const GOLDEN: &str = include_str!("../fixtures/item/registry_refs_golden.txt");

struct Golden {
    kind: ItemComponentKind,
    input: String,
    outcome: Outcome,
}

enum Outcome {
    Error(String),
    Value {
        json: String,
        nbt: Vec<u8>,
        wire: Vec<u8>,
        hash: i32,
    },
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn parse_fixture() -> (TestLookup, Vec<Golden>) {
    let mut ids: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
    let mut samples = Vec::new();
    let mut lines = GOLDEN.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("id ") {
            let [registry, name, id] = rest.split(' ').collect::<Vec<_>>()[..] else {
                panic!("malformed id line: {line}");
            };
            let registry = registry.strip_prefix("minecraft:").unwrap();
            let name = name.strip_prefix("minecraft:").unwrap();
            ids.entry(registry.to_owned())
                .or_default()
                .push((name.to_owned(), id.parse().unwrap()));
            continue;
        }
        let Some(rest) = line.strip_prefix("sample ") else {
            continue;
        };
        let (kind, input) = rest.split_once(' ').unwrap();
        let kind = ItemComponentKind::from_id(kind).unwrap_or_else(|| panic!("{kind}"));
        let mut fields: BTreeMap<&str, &str> = BTreeMap::new();
        while let Some(next) = lines.peek() {
            let Some(field) = next.strip_prefix("  ") else {
                break;
            };
            let (key, value) = field.split_once(' ').unwrap_or((field, ""));
            fields.insert(key, value);
            lines.next();
        }
        let outcome = match fields.get("error") {
            Some(message) => Outcome::Error((*message).to_owned()),
            None => Outcome::Value {
                json: fields["json"].to_owned(),
                nbt: hex(fields["nbt"]),
                wire: hex(fields["wire"]),
                hash: fields["hash"].parse().unwrap(),
            },
        };
        samples.push(Golden {
            kind,
            input: input.to_owned(),
            outcome,
        });
    }
    let mut lookup = TestLookup::new();
    for (registry, entries) in ids {
        let entries: Vec<(&str, u32)> = entries.iter().map(|(n, id)| (n.as_str(), *id)).collect();
        lookup.registry_with_ids(Box::leak(registry.into_boxed_str()), &entries);
    }
    (lookup, samples)
}

fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut child_tags: Vec<(String, NbtTag)> = compound
                .child_tags
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            child_tags.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(NbtCompound { child_tags })
        }
        NbtTag::List(items) => NbtTag::List(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn nbt_tree(bytes: &[u8]) -> NbtTag {
    sorted(mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).unwrap())
}

/// A one-entry list and a bare entry are the same holder set to vanilla and
/// write the same bytes, but `HolderSet::One` and `HolderSet::List` are
/// distinct values here, so read-backs are compared through their persistent
/// form rather than by `PartialEq`.
fn from_json(kind: ItemComponentKind, json: &str) -> Result<ItemComponentValue, String> {
    let mut d = serde_json::Deserializer::from_str(json);
    ItemComponentValue::deserialize_value(kind, &mut d).map_err(|e| e.to_string())
}

/// Vanilla's record codecs read only the keys they know, so the `extra` field
/// in that sample is accepted there; here a malformed datapack fails at load.
///
/// `FloatTag.valueOf` hands `-0.0f` the cached `ZERO`, so vanilla's NBT alone
/// loses the sign that its JSON and wire forms keep.
fn nbt_drops_negative_zero(input: &str) -> bool {
    input.contains("-0.0")
}

#[test]
fn every_golden_sample_matches_vanilla() {
    let (lookup, samples) = parse_fixture();
    assert_eq!(samples.len(), 62);
    let mut kinds_seen = std::collections::BTreeSet::new();
    for sample in &samples {
        let Golden {
            kind,
            input,
            outcome,
        } = sample;
        kinds_seen.insert(*kind);
        match outcome {
            Outcome::Error(message) => {
                let error = from_json(*kind, input)
                    .err()
                    .unwrap_or_else(|| panic!("{kind} accepted {input}, vanilla said: {message}"));
                let prefix = message
                    .split(" missed input")
                    .next()
                    .unwrap()
                    .split(" in {")
                    .next()
                    .unwrap();
                assert!(
                    error.starts_with(prefix),
                    "{kind} {input}: expected {prefix:?}, got {error:?}"
                );
            }
            Outcome::Value {
                json,
                nbt,
                wire,
                hash,
            } => {
                if input.contains(r#""extra":"#) {
                    let error = from_json(*kind, input).unwrap_err();
                    assert!(error.contains("unknown field `extra`"), "{kind}: {error}");
                    continue;
                }
                let value = from_json(*kind, input)
                    .unwrap_or_else(|e| panic!("{kind} rejected {input}: {e}"));
                let ours: serde_json::Value =
                    serde_json::from_str(&persistent_json(&value)).unwrap();
                let theirs: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(ours, theirs, "{kind} {input}: JSON");
                assert_eq!(
                    persistent_json(&from_json(*kind, json).unwrap()),
                    persistent_json(&value),
                    "{kind}: vanilla's JSON reads back the same"
                );

                if !nbt_drops_negative_zero(input) {
                    let mut our_nbt = Vec::new();
                    mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(&value), &mut our_nbt)
                        .unwrap();
                    assert_eq!(nbt_tree(&our_nbt), nbt_tree(nbt), "{kind} {input}: NBT");
                }

                let mut our_wire = Vec::new();
                value.encode_ctx_value(&lookup, &mut our_wire).unwrap();
                assert_eq!(
                    hex_string(&our_wire),
                    hex_string(wire),
                    "{kind} {input}: wire"
                );
                let mut r = &wire[..];
                let decoded = ItemComponentValue::decode_ctx_value(*kind, &lookup, &mut r).unwrap();
                assert!(r.is_empty(), "{kind}: trailing wire bytes");
                assert_eq!(
                    persistent_json(&decoded),
                    persistent_json(&value),
                    "{kind} {input}: wire decode"
                );

                assert_eq!(
                    hash_ops::hash(&PersistentValue(&value)).unwrap(),
                    *hash,
                    "{kind} {input}: hash"
                );
            }
        }
    }
    assert_eq!(kinds_seen.len(), 30);
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn a_lenient_stew_duration_falls_back_without_consuming_the_next_field() {
    let value = from_json(
        ItemComponentKind::SuspiciousStewEffects,
        r#"[{"id":"minecraft:speed","duration":{"nested":[1,2]}},{"id":"minecraft:haste","duration":7}]"#,
    )
    .unwrap();
    assert_eq!(
        persistent_json(&value),
        r#"[{"id":"minecraft:speed"},{"id":"minecraft:haste","duration":7}]"#
    );
}

#[test]
fn unknown_fields_are_rejected_at_load() {
    for (kind, json) in [
        (
            ItemComponentKind::AttributeModifiers,
            r#"[{"type":"minecraft:armor","id":"mcrs:x","amount":1,"operation":"add_value","extra":1}]"#,
        ),
        (ItemComponentKind::Tool, r#"{"rules":[],"speed":1}"#),
        (
            ItemComponentKind::Equippable,
            r#"{"slot":"head","sound":"x"}"#,
        ),
        (
            ItemComponentKind::PotionContents,
            r#"{"potion":"minecraft:water","color":1}"#,
        ),
        (
            ItemComponentKind::Bees,
            r#"[{"entity_data":{"id":"minecraft:pig"},"ticks_in_hive":1,"min_ticks_in_hive":1,"x":1}]"#,
        ),
    ] {
        let error = from_json(kind, json).unwrap_err();
        assert!(error.contains("unknown field"), "{kind}: {error}");
    }
}

#[test]
fn out_of_range_wire_ids_read_as_the_first_entry() {
    use mcrs_minecraft_protocol::entity::EquipmentSlot;
    use mcrs_minecraft_protocol::item::{AttributeDisplay, AttributeOperation, Equippable};

    let lookup = TestLookup::new();
    let head = from_json(ItemComponentKind::Equippable, r#"{"slot":"head"}"#).unwrap();
    let mut wire = Vec::new();
    head.encode_ctx_value(&lookup, &mut wire).unwrap();
    assert_eq!(wire[0], 4);
    wire[0] = 9;
    let decoded = ItemComponentValue::decode_ctx_value(
        ItemComponentKind::Equippable,
        &lookup,
        &mut &wire[..],
    )
    .unwrap();
    let ItemComponentValue::Equippable(Equippable { slot, .. }) = decoded else {
        unreachable!()
    };
    assert_eq!(slot, EquipmentSlot::MainHand);

    let entry = from_json(
        ItemComponentKind::AttributeModifiers,
        r#"[{"type":"minecraft:armor","id":"mcrs:x","amount":1,"operation":"add_multiplied_total","slot":"saddle","display":{"type":"hidden"}}]"#,
    )
    .unwrap();
    let mut wire = Vec::new();
    entry.encode_ctx_value(&lookup, &mut wire).unwrap();
    let len = wire.len();
    assert_eq!(&wire[len - 3..], &[2, 10, 1]);
    wire[len - 3] = 7;
    wire[len - 2] = 11;
    wire[len - 1] = 3;
    let decoded = ItemComponentValue::decode_ctx_value(
        ItemComponentKind::AttributeModifiers,
        &lookup,
        &mut &wire[..],
    )
    .unwrap();
    let ItemComponentValue::AttributeModifiers(modifiers) = decoded else {
        unreachable!()
    };
    assert_eq!(
        modifiers.0[0].modifier.operation,
        AttributeOperation::AddValue
    );
    assert_eq!(
        modifiers.0[0].slot,
        mcrs_minecraft_protocol::item::EquipmentSlotGroup::Any
    );
    assert_eq!(modifiers.0[0].display, AttributeDisplay::Default);
}
