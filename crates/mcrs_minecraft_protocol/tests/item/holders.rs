//! Values produced by the vanilla 26.3-snapshot-10 codecs for the kinds that
//! carry a holder or a consume effect, with the registry ids the vanilla
//! buffer resolved them against.

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

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

const GOLDEN: &str = include_str!("../fixtures/item/holders_golden.txt");

const KINDS: &[(&str, ItemComponentKind)] = &[
    ("break_sound", ItemComponentKind::BreakSound),
    ("consumable", ItemComponentKind::Consumable),
    ("death_protection", ItemComponentKind::DeathProtection),
    ("blocks_attacks", ItemComponentKind::BlocksAttacks),
    ("piercing_weapon", ItemComponentKind::PiercingWeapon),
    ("kinetic_weapon", ItemComponentKind::KineticWeapon),
    ("trim", ItemComponentKind::Trim),
    (
        "provides_trim_material",
        ItemComponentKind::ProvidesTrimMaterial,
    ),
    ("instrument", ItemComponentKind::Instrument),
    ("jukebox_playable", ItemComponentKind::JukeboxPlayable),
    ("banner_patterns", ItemComponentKind::BannerPatterns),
    ("painting_variant", ItemComponentKind::PaintingVariant),
];

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

struct Golden {
    lookup: TestLookup,
    samples: BTreeMap<String, BTreeMap<String, String>>,
}

fn golden() -> Golden {
    let mut lookup = TestLookup::new();
    let mut ids: BTreeMap<&str, Vec<(&str, u32)>> = BTreeMap::new();
    let mut samples: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for line in GOLDEN.lines() {
        let mut parts = line.splitn(3, ' ');
        let (label, key, value) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap_or(""),
        );
        if label == "id" {
            let (name, id) = value.split_once(' ').unwrap();
            let path = name.strip_prefix("minecraft:").unwrap();
            ids.entry(key)
                .or_default()
                .push((path, id.parse().unwrap()));
        } else {
            samples
                .entry(label.into())
                .or_default()
                .insert(key.into(), value.into());
        }
    }
    for (registry, entries) in ids {
        let registry: &'static str = Box::leak(registry.to_string().into_boxed_str());
        lookup.registry_with_ids(registry, &entries);
    }
    Golden { lookup, samples }
}

fn kind_of(label: &str) -> ItemComponentKind {
    KINDS
        .iter()
        .filter(|(prefix, _)| label.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, kind)| *kind)
        .unwrap_or_else(|| panic!("no kind for {label}"))
}

/// Vanilla's compound is a hash map, so its NBT bytes carry no reproducible
/// order; the trees are compared with every compound sorted.
fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut sorted_compound = NbtCompound::new();
            let mut entries: Vec<(String, NbtTag)> = compound.child_tags.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in entries {
                sorted_compound.child_tags.push((key, sorted(value)));
            }
            NbtTag::Compound(sorted_compound)
        }
        NbtTag::List(items) => NbtTag::List(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn nbt_tree(bytes: &[u8]) -> String {
    let tag: NbtTag =
        mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).expect("nbt");
    sorted(tag).to_string()
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
        value.encode_ctx_value(&lookup, &mut out).unwrap();
        assert_eq!(out, wire, "{label} wire");
        let mut r = &wire[..];
        let back = ItemComponentValue::decode_ctx_value(kind, &lookup, &mut r).unwrap();
        assert!(r.is_empty(), "{label} trailing bytes");
        assert_eq!(back, value, "{label} wire decode");
        checked += 1;
    }
    assert_eq!(checked, 33);
}

fn wire_only(label: &str, value: impl Into<ItemComponentValue>) {
    let Golden { lookup, samples } = golden();
    let value = value.into();
    let wire = hex(&samples[label]["wire"]);
    let mut out = Vec::new();
    value.encode_ctx_value(&lookup, &mut out).unwrap();
    assert_eq!(out, wire, "{label} wire");
    let mut r = &wire[..];
    let back = ItemComponentValue::decode_ctx_value(value.kind(), &lookup, &mut r).unwrap();
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
    for (input, kind) in [
        (
            r#"{"contact_cooldown_ticks":-1}"#,
            ItemComponentKind::KineticWeapon,
        ),
        (
            r#"{"sound_event":"minecraft:entity.item.break","use_duration":1.0,"range":1.0,"durability_damage":-1,"description":"x"}"#,
            ItemComponentKind::Instrument,
        ),
    ] {
        let mut d = serde_json::Deserializer::from_str(input);
        let error = ItemComponentValue::deserialize_value(kind, &mut d).unwrap_err();
        assert!(
            error.to_string().contains("Value must be non-negative: -1"),
            "{input}: {error}"
        );
    }
    let bad_type = [5u8];
    let error = ConsumeEffect::decode_ctx(&lookup, &mut &bad_type[..]).unwrap_err();
    assert_eq!(error.to_string(), "unknown consume effect type 5");
}
