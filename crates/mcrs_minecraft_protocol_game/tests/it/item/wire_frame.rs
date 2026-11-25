use std::collections::BTreeSet;

use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item_stack::WireStack;
use mcrs_minecraft_item_stack::WireStackWithLength;
use mcrs_minecraft_protocol::{Decode, Encode, ErrorKind, VarInt};
use mcrs_minecraft_protocol_game::item::DelimitedStack;
use mcrs_minecraft_protocol_game::item::ProtoStack;
use mcrs_minecraft_protocol_game::item::for_each_data_component;
use mcrs_minecraft_registries::Wire;
use serde::Deserialize;

use crate::item::harness::{hex, lookup, samples, wire};

const RECORD_KINDS: [(&str, DataComponentType); 36] = [
    ("attack_range", DataComponentType::AttackRange),
    ("banner_patterns", DataComponentType::BannerPatterns),
    ("block_state", DataComponentType::BlockState),
    ("blocks_attacks", DataComponentType::BlocksAttacks),
    ("break_sound", DataComponentType::BreakSound),
    ("brewing", DataComponentType::BrewingFuel),
    ("bucket", DataComponentType::BucketEntityData),
    ("cmd", DataComponentType::CustomModelData),
    ("compostable", DataComponentType::Compostable),
    ("consumable", DataComponentType::Consumable),
    ("cooking", DataComponentType::CookingFuel),
    ("cooldown", DataComponentType::UseCooldown),
    ("death_protection", DataComponentType::DeathProtection),
    ("debug", DataComponentType::DebugStickState),
    ("explosion", DataComponentType::FireworkExplosion),
    ("fireworks", DataComponentType::Fireworks),
    ("food", DataComponentType::Food),
    ("instrument", DataComponentType::Instrument),
    ("item_model", DataComponentType::ItemModel),
    ("jukebox_playable", DataComponentType::JukeboxPlayable),
    ("kinetic_weapon", DataComponentType::KineticWeapon),
    ("lodestone", DataComponentType::LodestoneTracker),
    ("loot", DataComponentType::ContainerLoot),
    ("map_dec", DataComponentType::MapDecorations),
    ("note_block_sound", DataComponentType::NoteBlockSound),
    ("painting_variant", DataComponentType::PaintingVariant),
    ("piercing_weapon", DataComponentType::PiercingWeapon),
    ("profile", DataComponentType::Profile),
    (
        "provides_trim_material",
        DataComponentType::ProvidesTrimMaterial,
    ),
    ("recipes", DataComponentType::Recipes),
    ("sign", DataComponentType::SignTextFront),
    ("tooltip_style", DataComponentType::TooltipStyle),
    ("tooltip", DataComponentType::TooltipDisplay),
    ("trim", DataComponentType::Trim),
    ("use_effects", DataComponentType::UseEffects),
    ("weapon", DataComponentType::Weapon),
];

#[derive(Deserialize)]
struct Plain {
    values: Vec<KindWire>,
    decodes: Vec<KindWire>,
}

#[derive(Deserialize)]
struct Nested {
    cases: Vec<KindWire>,
}

#[derive(Deserialize)]
struct KindWire {
    kind: String,
    wire: String,
}

struct Case {
    kind: DataComponentType,
    value: Vec<u8>,
}

fn kind(name: &str) -> DataComponentType {
    DataComponentType::read(name).unwrap_or_else(|| panic!("{name} is not a component type"))
}

fn record_kind(label: &str) -> DataComponentType {
    if let Some(rest) = label.strip_prefix("writ") {
        return if rest.starts_with("ten") {
            DataComponentType::WrittenBookContent
        } else {
            DataComponentType::WritableBookContent
        };
    }
    RECORD_KINDS
        .iter()
        .filter(|(prefix, _)| label.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, kind)| *kind)
        .unwrap_or_else(|| panic!("the record {label} names no component type"))
}

fn json_cases(rows: Vec<KindWire>) -> impl Iterator<Item = Case> {
    rows.into_iter().map(|row| Case {
        kind: kind(&row.kind),
        value: hex(&row.wire),
    })
}

fn fixture_cases() -> Vec<Case> {
    let plain: Plain = serde_json::from_str(&fixture("item_plain").text()).unwrap();
    let nested: Nested = serde_json::from_str(&fixture("item_nested").text()).unwrap();
    let mut cases: Vec<Case> = json_cases(plain.values)
        .chain(json_cases(plain.decodes))
        .chain(json_cases(nested.cases))
        .collect();
    let refs = fixture("item_registry_refs").text();
    let mut current = None;
    for line in refs.lines() {
        if let Some(rest) = line.strip_prefix("sample ") {
            current = rest.split_once(' ').map(|(name, _)| kind(name));
        } else if let (Some(kind), Some(wire)) = (current, line.strip_prefix("  wire ")) {
            cases.push(Case {
                kind,
                value: hex(wire),
            });
        }
    }
    for (name, separator) in [("item_records", " wire = "), ("item_holders", " wire ")] {
        for line in fixture(name).text().lines() {
            if let Some((label, wire)) = line.split_once(separator)
                && !label.contains(' ')
            {
                cases.push(Case {
                    kind: record_kind(label),
                    value: hex(wire),
                });
            }
        }
    }
    cases
}

macro_rules! sample_cases {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        fn sample_cases() -> Vec<Case> {
            let lookup = lookup();
            let mut cases = Vec::new();
            $(
                for value in samples::<mcrs_minecraft_protocol_game::item::$ty>() {
                    cases.push(Case {
                        kind: DataComponentType::$kind,
                        value: wire(&lookup, &value),
                    });
                }
            )*
            cases
        }
    };
}

for_each_data_component!(sample_cases);

fn var_int(value: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    VarInt(value).encode(&mut bytes).unwrap();
    bytes
}

fn stack_with(kind: DataComponentType, value: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x01, 0x01, 0x01, 0x00];
    bytes.extend(var_int(i32::from(kind.id().number())));
    bytes.extend_from_slice(value);
    bytes.extend_from_slice(&[0xab, 0xcd]);
    bytes
}

fn agrees_with_the_old_decoder(case: &Case) -> bool {
    let bytes = stack_with(case.kind, &case.value);
    let mut old = &bytes[..];
    let Ok(proto) = ProtoStack::<Wire>::decode(&mut old) else {
        return false;
    };
    let mut new = &bytes[..];
    let frame = WireStack::decode(&mut new).unwrap_or_else(|error| {
        panic!(
            "{:?}: the frame refuses {bytes:02x?} that the old decoder reads: {error}",
            case.kind
        )
    });
    assert_eq!(new.len(), old.len(), "{:?}: end of {bytes:02x?}", case.kind);
    let mut encoded = Vec::new();
    frame.encode(&mut encoded).unwrap();
    assert_eq!(encoded, bytes[..bytes.len() - old.len()], "{:?}", case.kind);

    let mut delimited = Vec::new();
    DelimitedStack(proto).encode(&mut delimited).unwrap();
    let mut input = &delimited[..];
    let frame = WireStackWithLength::decode(&mut input).unwrap();
    assert!(input.is_empty(), "{:?}: end of {delimited:02x?}", case.kind);
    assert_eq!(frame.bytes(), delimited, "{:?}", case.kind);
    true
}

fn covered(cases: &[Case]) -> BTreeSet<DataComponentType> {
    cases
        .iter()
        .filter(|case| agrees_with_the_old_decoder(case))
        .map(|case| case.kind)
        .collect()
}

#[test]
fn the_frame_finds_the_bounds_that_the_old_decoder_finds_for_every_component_type() {
    let from_fixtures = covered(&fixture_cases());
    let from_samples = covered(&sample_cases());
    let uncovered: Vec<DataComponentType> = DataComponentType::ALL
        .iter()
        .copied()
        .filter(|kind| !from_fixtures.contains(kind) && !from_samples.contains(kind))
        .collect();
    assert_eq!(uncovered, [], "component types that no case covers");
}

#[derive(Clone, Copy)]
enum Form {
    Bare,
    WithLength,
}

#[test]
fn the_frame_refuses_bytes_that_have_no_bounds() {
    let food = var_int(i32::from(DataComponentType::Food.id().number()));
    let rows: [(&str, Form, Vec<u8>, bool); 5] = [
        (
            "truncated value",
            Form::Bare,
            [&[0x01, 0x01, 0x01, 0x00][..], &food, &[0x04, 0x3f]].concat(),
            true,
        ),
        (
            "unknown type id",
            Form::Bare,
            vec![0x01, 0x01, 0x01, 0x00, 0x7a],
            false,
        ),
        (
            "negative entry count",
            Form::Bare,
            [&[0x01, 0x01][..], &var_int(-1), &[0x00]].concat(),
            false,
        ),
        (
            "negative length",
            Form::WithLength,
            [&[0x01, 0x01, 0x01, 0x00][..], &food, &var_int(-1)].concat(),
            false,
        ),
        (
            "length past the end",
            Form::WithLength,
            [&[0x01, 0x01, 0x01, 0x00][..], &food, &[0x06, 0x04]].concat(),
            true,
        ),
    ];
    for (name, form, bytes, end) in rows {
        let error = match form {
            Form::Bare => WireStack::decode(&mut &bytes[..]).map(drop),
            Form::WithLength => WireStackWithLength::decode(&mut &bytes[..]).map(drop),
        }
        .expect_err(name);
        assert_eq!(
            matches!(error.kind(), ErrorKind::UnexpectedEnd),
            end,
            "{name}: {error}"
        );
    }
}

#[derive(Clone, Copy)]
enum Nesting {
    ContainerSlot,
    BundleList,
    BlockPredicate,
}

fn nested_value(nesting: Nesting, kind: &[u8], levels: usize) -> Vec<u8> {
    let mut value = match nesting {
        Nesting::ContainerSlot | Nesting::BundleList => vec![0x01, 0x01, 0x00, 0x00],
        Nesting::BlockPredicate => vec![0x00, 0x00, 0x00, 0x00, 0x00],
    };
    for level in 1..=levels {
        value = match nesting {
            Nesting::ContainerSlot => [&[0x01, 0x01][..], &value].concat(),
            Nesting::BundleList | Nesting::BlockPredicate => [&[0x01][..], &value].concat(),
        };
        if level < levels {
            value = match nesting {
                Nesting::ContainerSlot | Nesting::BundleList => {
                    [&[0x01, 0x01, 0x01, 0x00][..], kind, &value].concat()
                }
                Nesting::BlockPredicate => {
                    [&[0x00, 0x00, 0x00, 0x01][..], kind, &value, &[0x00]].concat()
                }
            };
        }
    }
    value
}

#[test]
fn the_frame_refuses_a_stack_nested_past_the_depth_limit() {
    let rows = [
        (Nesting::ContainerSlot, DataComponentType::Container, 127),
        (Nesting::BundleList, DataComponentType::BundleContents, 170),
        (Nesting::BlockPredicate, DataComponentType::CanPlaceOn, 511),
    ];
    for (nesting, kind, limit) in rows {
        let id = var_int(i32::from(kind.id().number()));
        for (levels, fits) in [(limit, true), (limit + 1, false)] {
            let bytes = stack_with(kind, &nested_value(nesting, &id, levels));
            let mut input = &bytes[..];
            let result = WireStack::decode(&mut input);
            assert_eq!(result.is_ok(), fits, "{kind:?} nested {levels} levels");
            if fits {
                assert_eq!(input, [0xab, 0xcd], "{kind:?} nested {levels} levels");
            }
        }
    }
}
