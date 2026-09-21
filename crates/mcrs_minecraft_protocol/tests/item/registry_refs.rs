use mcrs_minecraft_protocol::item::EncodeCtx;
use mcrs_minecraft_protocol::item::decode_component_value;
use std::collections::BTreeMap;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::{ItemComponentKind, ItemComponentValue, hash_ops};

use crate::harness::{TestLookup, from_nbt, hex, json_value, nbt_tree, persistent_json};

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

fn parse_fixture() -> (TestLookup, Vec<Golden>) {
    let mut samples = Vec::new();
    let mut lines = GOLDEN.lines().peekable();
    while let Some(line) = lines.next() {
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
    (TestLookup::with_id_lines(GOLDEN), samples)
}

fn from_json(kind: ItemComponentKind, json: &str) -> Result<ItemComponentValue, String> {
    let mut d = serde_json::Deserializer::from_str(json);
    ItemComponentValue::deserialize_value(kind, &mut d).map_err(|e| e.to_string())
}

/// Vanilla's record codecs read only the keys they know, so the `extra` field
/// in that sample is accepted there; here a malformed datapack fails at load.
#[test]
fn every_golden_sample_matches_vanilla() {
    let (lookup, samples) = parse_fixture();
    assert_eq!(samples.len(), 78);
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
                let theirs: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(json_value(&value), theirs, "{kind} {input}: JSON");
                assert_eq!(
                    persistent_json(&from_json(*kind, json).unwrap()),
                    persistent_json(&value),
                    "{kind}: vanilla's JSON reads back the same"
                );

                let mut our_nbt = Vec::new();
                mcrs_minecraft_nbt::to_bytes_unnamed(&value, &mut our_nbt).unwrap();
                assert_eq!(nbt_tree(&our_nbt), nbt_tree(nbt), "{kind} {input}: NBT");

                let mut our_wire = Vec::new();
                value.encode_ctx(&lookup, &mut our_wire).unwrap();
                assert_eq!(our_wire, *wire, "{kind} {input}: wire");
                let mut r = &wire[..];
                let decoded = decode_component_value(*kind, &lookup, &mut r).unwrap();
                assert!(r.is_empty(), "{kind}: trailing wire bytes");
                assert_eq!(
                    persistent_json(&decoded),
                    persistent_json(&value),
                    "{kind} {input}: wire decode"
                );

                assert_eq!(
                    hash_ops::hash(&value).unwrap(),
                    *hash,
                    "{kind} {input}: hash"
                );
            }
        }
    }
    assert_eq!(kinds_seen.len(), 30);
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
    head.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(wire[0], 4);
    wire[0] = 9;
    let decoded =
        decode_component_value(ItemComponentKind::Equippable, &lookup, &mut &wire[..]).unwrap();
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
    entry.encode_ctx(&lookup, &mut wire).unwrap();
    let len = wire.len();
    assert_eq!(&wire[len - 3..], &[2, 10, 1]);
    wire[len - 3] = 7;
    wire[len - 2] = 11;
    wire[len - 1] = 3;
    let decoded = decode_component_value(
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

#[test]
fn a_stack_with_several_enchantments_survives_the_registry_free_pass() {
    use mcrs_minecraft_protocol::Decode;
    use mcrs_minecraft_protocol::item::{
        ComponentPatch, Enchantments, EncodeCtx, ProtoStack, RawStack,
    };
    use mcrs_minecraft_registry::ItemId;

    let lookup = TestLookup::new();
    let enchantments = from_json(
        ItemComponentKind::Enchantments,
        r#"{"minecraft:sharpness":5,"minecraft:unbreaking":3}"#,
    )
    .unwrap();
    let slot = ProtoStack::new(
        ItemId(2),
        1,
        ComponentPatch {
            added: vec![enchantments],
            removed: Vec::new(),
        },
    );
    let mut wire = Vec::new();
    slot.encode_ctx(&lookup, &mut wire).unwrap();
    let raw = RawStack::decode(&mut &wire[..]).unwrap();
    assert_eq!(raw.0, wire);
    assert_eq!(raw.resolve(&lookup).unwrap(), slot);

    let ItemComponentValue::Enchantments(expected) = &slot.components.added[0] else {
        unreachable!()
    };
    let reversed = Enchantments(expected.0.iter().rev().cloned().collect());
    assert_eq!(&reversed, expected);
    assert_ne!(Enchantments(vec![expected.0[0].clone()]), *expected);
}

#[test]
fn wire_enchantment_levels_follow_the_constructor_not_the_codec() {
    let lookup = TestLookup::new();
    let decode = |wire: &[u8]| {
        let mut r = wire;
        decode_component_value(ItemComponentKind::Enchantments, &lookup, &mut r)
            .map(|value| persistent_json(&value))
            .map_err(|e| e.to_string())
    };
    assert_eq!(decode(&[1, 0, 0]).unwrap(), r#"{"minecraft:sharpness":0}"#);
    assert_eq!(
        decode(&[2, 0, 5, 0, 3]).unwrap(),
        r#"{"minecraft:sharpness":3}"#
    );
    assert_eq!(
        decode(&[1, 0, 0x80, 0x02]).unwrap_err(),
        "Enchantment minecraft:sharpness has invalid level 256"
    );
    assert!(
        from_json(
            ItemComponentKind::Enchantments,
            r#"{"minecraft:sharpness":0}"#
        )
        .unwrap_err()
        .starts_with("Value 0 outside of range [1:255]")
    );
}

#[test]
fn nbt_floats_keep_vanillas_number_semantics() {
    use mcrs_minecraft_nbt::tag::NbtTag;

    fn from_tag(kind: ItemComponentKind, tag: NbtTag) -> Result<String, String> {
        let mut bytes = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&tag, &mut bytes).unwrap();
        let mut cursor = std::io::Cursor::new(&bytes[..]);
        let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
        ItemComponentValue::deserialize_value(kind, &mut d)
            .map(|value| persistent_json(&value))
            .map_err(|e| e.to_string())
    }
    fn compound(entries: Vec<(&str, NbtTag)>) -> NbtTag {
        NbtTag::Compound(NbtCompound {
            child_tags: entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        })
    }
    let tool = |speed: f32| {
        compound(vec![(
            "rules",
            NbtTag::List(vec![compound(vec![
                ("blocks", NbtTag::String("minecraft:stone".into())),
                ("speed", NbtTag::Float(speed)),
            ])]),
        )])
    };
    for speed in [f32::INFINITY, f32::NAN, -0.0] {
        let error = from_tag(ItemComponentKind::Tool, tool(speed)).unwrap_err();
        assert!(error.contains("Value must be positive: "), "{error}");
    }
    assert!(
        from_tag(ItemComponentKind::Tool, tool(f32::MAX))
            .unwrap()
            .contains(r#""speed":3.4028235e"#)
    );

    let visibility = |value: f32| {
        compound(vec![
            (
                "targeting_entity_types",
                NbtTag::String("minecraft:zombie".into()),
            ),
            ("visibility", NbtTag::Float(value)),
        ])
    };
    assert_eq!(
        from_tag(ItemComponentKind::MobVisibility, visibility(-0.0)).unwrap(),
        r#"{"targeting_entity_types":"minecraft:zombie","visibility":0.0}"#
    );
    assert!(from_tag(ItemComponentKind::MobVisibility, visibility(f32::NAN)).is_err());
    assert!(from_tag(ItemComponentKind::MobVisibility, visibility(0.0)).is_ok());
    assert!(
        from_json(
            ItemComponentKind::MobVisibility,
            r#"{"targeting_entity_types":"minecraft:zombie","visibility":-0.0}"#
        )
        .unwrap_err()
        .starts_with("Value must be within range [0.0;10.0]: -0.0")
    );

    let color = |tag: NbtTag| compound(vec![("custom_color", tag)]);
    assert_eq!(
        from_tag(ItemComponentKind::PotionContents, color(NbtTag::Float(1.9))).unwrap(),
        r#"{"custom_color":1}"#
    );
    assert_eq!(
        from_tag(
            ItemComponentKind::PotionContents,
            color(NbtTag::Long(2147483648))
        )
        .unwrap(),
        r#"{"custom_color":-2147483648}"#
    );
    assert_eq!(
        from_tag(
            ItemComponentKind::PotionContents,
            color(NbtTag::Double(3e9))
        )
        .unwrap(),
        r#"{"custom_color":2147483647}"#
    );

    let stew = |tag: NbtTag| {
        NbtTag::List(vec![compound(vec![
            ("id", NbtTag::String("minecraft:speed".into())),
            ("duration", tag),
        ])])
    };
    assert_eq!(
        from_tag(
            ItemComponentKind::SuspiciousStewEffects,
            stew(NbtTag::Double(3e9))
        )
        .unwrap(),
        r#"[{"id":"minecraft:speed","duration":2147483647}]"#
    );
    assert_eq!(
        from_tag(
            ItemComponentKind::SuspiciousStewEffects,
            stew(NbtTag::Byte(1))
        )
        .unwrap(),
        r#"[{"id":"minecraft:speed","duration":1}]"#
    );
    assert_eq!(
        from_tag(
            ItemComponentKind::SuspiciousStewEffects,
            stew(NbtTag::List(vec![NbtTag::Int(1), NbtTag::Int(2)]))
        )
        .unwrap(),
        r#"[{"id":"minecraft:speed"}]"#
    );
}

#[test]
fn a_negative_zero_from_the_wire_reloads_from_its_own_save() {
    let (lookup, _) = parse_fixture();
    let wire = [0x02, 0x9a, 0x01, 0x80, 0x00, 0x00, 0x00];
    let value =
        decode_component_value(ItemComponentKind::MobVisibility, &lookup, &mut &wire[..]).unwrap();
    assert!(persistent_json(&value).ends_with(r#""visibility":-0.0}"#));
    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&value, &mut nbt).unwrap();
    assert!(nbt.ends_with(&[0x00, 0x00, 0x00, 0x00, 0x00]));
    let reloaded = from_nbt(ItemComponentKind::MobVisibility, &nbt);
    assert!(persistent_json(&reloaded).ends_with(r#""visibility":0.0}"#));
}

#[test]
fn non_finite_floats_cross_the_wire() {
    let (lookup, _) = parse_fixture();
    for (kind, wire, json) in [
        (
            ItemComponentKind::Tool,
            hex("010201013f800000007f8000000101"),
            r#"{"rules":[{"blocks":"minecraft:stone","speed":1.0}],"default_mining_speed":null}"#,
        ),
        (
            ItemComponentKind::AttributeModifiers,
            hex("011e066d6372733a787ff0000000000000000000"),
            r#"[{"type":"minecraft:scale","id":"mcrs:x","amount":null,"operation":"add_value"}]"#,
        ),
    ] {
        let mut r = &wire[..];
        let value = decode_component_value(kind, &lookup, &mut r).unwrap();
        assert!(r.is_empty());
        assert_eq!(persistent_json(&value), json, "{kind}");
        let mut encoded = Vec::new();
        value.encode_ctx(&lookup, &mut encoded).unwrap();
        assert_eq!(encoded, wire, "{kind}");
    }
    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(
        &from_json(
            ItemComponentKind::Tool,
            r#"{"rules":[],"default_mining_speed":1e40}"#,
        )
        .unwrap(),
        &mut nbt,
    )
    .unwrap();
    assert_eq!(
        nbt_tree(&nbt),
        nbt_tree(&hex(
            "0a05001464656661756c745f6d696e696e675f73706565647f80000009000572756c6573000000000000"
        ))
    );
}

#[test]
fn two_spellings_of_one_enchantment_are_still_a_duplicate() {
    assert_eq!(
        persistent_json(
            &from_json(
                ItemComponentKind::Enchantments,
                r#"{"minecraft:sharpness":1,"minecraft:unbreaking":3,"minecraft:sharpness":2}"#
            )
            .unwrap()
        ),
        r#"{"minecraft:sharpness":2,"minecraft:unbreaking":3}"#
    );
    assert!(
        from_json(
            ItemComponentKind::Enchantments,
            r#"{"minecraft:sharpness":1,"sharpness":2}"#
        )
        .unwrap_err()
        .starts_with("Duplicate entry for key: minecraft:sharpness")
    );
}

#[test]
fn a_one_entry_list_is_the_bare_entry() {
    use mcrs_minecraft_core::HolderSet;
    use mcrs_minecraft_protocol::item::DamageResistant;

    let lookup = TestLookup::new();
    let bare = from_json(
        ItemComponentKind::DamageResistant,
        r#"{"types":"minecraft:lava"}"#,
    )
    .unwrap();
    let list = from_json(
        ItemComponentKind::DamageResistant,
        r#"{"types":["minecraft:lava"]}"#,
    )
    .unwrap();
    assert_eq!(bare, list);
    let mut wire = Vec::new();
    bare.encode_ctx(&lookup, &mut wire).unwrap();
    let decoded =
        decode_component_value(ItemComponentKind::DamageResistant, &lookup, &mut &wire[..])
            .unwrap();
    assert_eq!(decoded, bare);
    let ItemComponentValue::DamageResistant(DamageResistant { types }) = decoded else {
        unreachable!()
    };
    assert!(matches!(types, HolderSet::One(_)));
}
