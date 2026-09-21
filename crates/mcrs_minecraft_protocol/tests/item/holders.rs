use mcrs_minecraft_protocol::item::EncodeCtx;
use mcrs_minecraft_protocol::item::decode_component_value;
use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::{
    Consumable, ConsumeEffect, ConsumeEffectType, DecodeCtx, Holder, HolderWireOnly,
    ItemComponentKind, ItemComponentValue, JukeboxPlayable, JukeboxSong, PaintingVariant,
    PaintingVariantValue, SoundEvent, hash_ops,
};
use mcrs_minecraft_protocol::text::Text;

use crate::harness::{PersistentValue, TestLookup, from_json, hex, nbt_tree, persistent_json};

const GOLDEN: &str = include_str!("../fixtures/item/holders_golden.txt");

struct Golden {
    lookup: TestLookup,
    samples: BTreeMap<String, BTreeMap<String, String>>,
}

fn golden() -> Golden {
    let mut samples: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for line in GOLDEN.lines().filter(|line| !line.starts_with("id ")) {
        let mut parts = line.splitn(3, ' ');
        let (label, key, value) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap_or(""),
        );
        samples
            .entry(label.into())
            .or_default()
            .insert(key.into(), value.into());
    }
    Golden {
        lookup: TestLookup::with_id_lines(GOLDEN),
        samples,
    }
}

fn kind_of(label: &str) -> ItemComponentKind {
    std::iter::successors(Some(label), |l| l.rsplit_once('_').map(|(head, _)| head))
        .find_map(|l| {
            ItemComponentKind::from_id(l)
                .or_else(|| ItemComponentKind::from_id(&l.replace('_', "/")))
        })
        .unwrap_or_else(|| panic!("no kind for {label}"))
}

#[test]
fn every_persistent_sample_matches_vanilla_in_json_nbt_hash_and_wire() {
    let Golden { lookup, samples } = golden();
    let mut checked = 0;
    for (label, fields) in &samples {
        let Some(input) = fields.get("in") else {
            continue;
        };
        let kind = kind_of(label);
        let value = from_json(kind, input);
        assert_eq!(&persistent_json(&value), &fields["json"], "{label} json");
        assert_eq!(from_json(kind, &fields["json"]), value, "{label} reparse");

        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(&value), &mut nbt).unwrap();
        assert_eq!(
            nbt_tree(&nbt),
            nbt_tree(&hex(&fields["nbt"])),
            "{label} nbt"
        );

        let hash: i32 = fields["hash"].parse().unwrap();
        assert_eq!(
            hash_ops::hash(&PersistentValue(&value)).unwrap(),
            hash,
            "{label} hash"
        );

        let wire = hex(&fields["wire"]);
        let mut out = Vec::new();
        value.encode_ctx(&lookup, &mut out).unwrap();
        assert_eq!(out, wire, "{label} wire");
        let mut r = &wire[..];
        let back = decode_component_value(kind, &lookup, &mut r).unwrap();
        assert!(r.is_empty(), "{label} trailing bytes");
        assert_eq!(back, value, "{label} wire decode");
        checked += 1;
    }
    assert_eq!(checked, 39);
}

fn wire_only(label: &str, value: impl Into<ItemComponentValue>) {
    let Golden { lookup, samples } = golden();
    let value = value.into();
    let wire = hex(&samples[label]["wire"]);
    let mut out = Vec::new();
    value.encode_ctx(&lookup, &mut out).unwrap();
    assert_eq!(out, wire, "{label} wire");
    let mut r = &wire[..];
    let back = decode_component_value(value.kind(), &lookup, &mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(back, value);
    let mut s = serde_json::Serializer::new(Vec::new());
    assert!(
        value.serialize_value(&mut s).is_err(),
        "{label} has no persistent form"
    );
}

#[test]
fn reference_only_kinds_still_carry_the_entry_inline_on_the_wire() {
    wire_only(
        "jukebox_playable_direct",
        JukeboxPlayable(HolderWireOnly(Holder::Direct(JukeboxSong {
            sound_event: Holder::reference(ResourceLocation::minecraft("entity.item.break")),
            description: Text::text("Song"),
            length_in_seconds: 12.5,
            comparator_output: Bounded(7),
        }))),
    );
    wire_only(
        "jukebox_playable_direct_sound",
        JukeboxPlayable(HolderWireOnly(Holder::Direct(JukeboxSong {
            sound_event: Holder::Direct(SoundEvent {
                sound_id: ResourceLocation::new("mcrs", "song"),
                range: None,
            }),
            description: Text::translate("song.mcrs", Vec::new()),
            length_in_seconds: 1.0,
            comparator_output: Bounded(0),
        }))),
    );
    wire_only(
        "painting_variant_direct",
        PaintingVariant(HolderWireOnly(Holder::Direct(PaintingVariantValue {
            width: Bounded(2),
            height: Bounded(1),
            asset_id: ResourceLocation::new("mcrs", "art"),
            title: Some(Text::text("T")),
            author: None,
        }))),
    );
    wire_only(
        "painting_variant_direct_both",
        PaintingVariant(HolderWireOnly(Holder::Direct(PaintingVariantValue {
            width: Bounded(16),
            height: Bounded(16),
            asset_id: ResourceLocation::new("mcrs", "big"),
            title: None,
            author: Some(Text::translate("author.mcrs", Vec::new())),
        }))),
    );
    let inline =
        serde_json::from_str::<PaintingVariant>(r#"{"width":1,"height":1,"asset_id":"mcrs:a"}"#);
    assert!(inline.is_err(), "inline painting variant is not persistent");
}

#[test]
fn consume_effect_ids_are_the_registry_protocol_ids() {
    let report: BTreeMap<String, serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let entries = &report["minecraft:consume_effect_type"]["entries"];
    assert_eq!(
        entries.as_object().unwrap().len(),
        ConsumeEffectType::ALL.len()
    );
    for kind in ConsumeEffectType::ALL {
        assert_eq!(
            entries[kind.id()]["protocol_id"],
            kind as u8,
            "{}",
            kind.id()
        );
    }
}

#[test]
fn errors_read_like_vanilla() {
    let Golden { lookup, .. } = golden();
    for (input, message) in [
        (
            r#"{"type":"minecraft:apply_effects","effects":[],"probability":1.5}"#,
            "Value 1.5 outside of range [0.0:1.0]",
        ),
        (
            r#"{"type":"minecraft:teleport_randomly","diameter":0.0}"#,
            "Value must be positive: 0.0",
        ),
        (
            r#"{"type":"minecraft:teleport_randomly","diameter":-0.0}"#,
            "Value must be positive: -0.0",
        ),
        (r#"{"type":"minecraft:nope"}"#, "unknown variant"),
        (
            r#"{"type":"minecraft:teleport_randomly","extra":1}"#,
            "unknown field",
        ),
    ] {
        let error = serde_json::from_str::<ConsumeEffect>(input).unwrap_err();
        assert!(error.to_string().contains(message), "{input}: {error}");
    }
    let unit_with_extras = serde_json::from_str::<ConsumeEffect>(
        r#"{"type":"minecraft:clear_all_effects","extra":1}"#,
    );
    assert_eq!(unit_with_extras.unwrap(), ConsumeEffect::ClearAllEffects);
    let error = serde_json::from_str::<Consumable>(r#"{"consume_seconds":-1}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Value must be non-negative: -1.0"),
        "{error}"
    );
    for (input, kind, message) in [
        (
            r#"{"contact_cooldown_ticks":-1}"#,
            ItemComponentKind::KineticWeapon,
            "Value must be non-negative: -1",
        ),
        (
            r#"{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":1.0,"durability_damage":-1,"description":"x"}"#,
            ItemComponentKind::Instrument,
            "Value must be non-negative: -1",
        ),
        (
            r#"{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":3.5e38,"durability_damage":1,"description":"x"}"#,
            ItemComponentKind::Instrument,
            "Value must be positive: Infinity",
        ),
        (
            r#"{"sound_event":"minecraft:entity.item.break","use_duration":-3.5e38,"range":1.0,"durability_damage":1,"description":"x"}"#,
            ItemComponentKind::Instrument,
            "Value must be non-negative: -Infinity",
        ),
    ] {
        let mut d = serde_json::Deserializer::from_str(input);
        let error = ItemComponentValue::deserialize_value(kind, &mut d).unwrap_err();
        assert!(error.to_string().contains(message), "{input}: {error}");
    }
    let bad_type = [5u8];
    let error = ConsumeEffect::decode_ctx(&lookup, &mut &bad_type[..]).unwrap_err();
    assert_eq!(error.to_string(), "unknown consume effect type 5");
}

#[test]
fn lenient_range_reads_any_nbt_number_and_no_json_non_number() {
    let mut compound = NbtCompound::new();
    compound
        .child_tags
        .push(("sound_id".into(), NbtTag::String("mcrs:custom".into())));
    compound.child_tags.push(("range".into(), NbtTag::Byte(1)));
    let event: SoundEvent = mcrs_minecraft_nbt::from_tag(NbtTag::Compound(compound)).unwrap();
    assert_eq!(event.range, Some(1.0));
    for input in [
        r#"{"sound_id":"mcrs:custom","range":false}"#,
        r#"{"sound_id":"mcrs:custom","range":{"deep":[1,[2]]}}"#,
    ] {
        let event: SoundEvent = serde_json::from_str(input).unwrap();
        assert_eq!(event.range, None, "{input}");
    }
}
