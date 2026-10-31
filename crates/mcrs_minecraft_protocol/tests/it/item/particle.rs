use crate::item::harness::WireCodec;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf, TagKeyBuf};
use std::collections::BTreeMap;
use std::sync::LazyLock;

use glam::DVec3;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_particle::keys::ParticleType;
use mcrs_minecraft_particle::{
    BlockParticle, BlockStateValue, ColorParticle, DustParticle, ItemParticle, ParticleOptions,
    ParticleScale, PositionSource, SpellParticle, TrailParticle, VibrationParticle,
};
use mcrs_minecraft_protocol::item::{ArgbInt, ComponentPatch, RgbInt, Template};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundLevelParticles, ParticleRandomization,
};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registries::{FromWire, Wire};
use mcrs_voxel::BlockPos;

use crate::item::harness::{block_lookup, hex, named, read_json};

static GOLDEN: LazyLock<String> = LazyLock::new(|| fixture("particles").text());

pub(crate) fn golden(key: &str) -> &'static str {
    let prefix = format!("{key} = ");
    GOLDEN
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no golden value {key}"))
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn wire(particle: &ParticleOptions) -> Vec<u8> {
    let mut out = Vec::new();
    particle.encode_checked(&block_lookup(), &mut out).unwrap();
    out
}

/// Decodes the golden wire bytes, checks they re-encode byte for byte, and
/// that the golden JSON parses to the same value and is written back as it
/// was read.
fn check(label: &str) -> ParticleOptions {
    let bytes = hex(golden(&format!("{label}.wire")));
    let mut r = &bytes[..];
    let decoded = ParticleOptions::decode_checked(&block_lookup(), &mut r).unwrap();
    assert!(r.is_empty(), "{label}: {} trailing bytes", r.len());
    assert_eq!(to_hex(&wire(&decoded)), to_hex(&bytes), "{label}: wire");

    let json = golden(&format!("{label}.json"));
    let parsed: ParticleOptions = read_json(json).unwrap();
    assert_eq!(parsed, decoded, "{label}: json {json}");
    let back: serde_json::Value = serde_json::to_value(named(&decoded)).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(back, vanilla, "{label}: json round trip");

    let unchecked = ParticleOptions::<Wire>::decode(&mut &bytes[..]).unwrap();
    let mut out = Vec::new();
    unchecked.encode(&mut out).unwrap();
    assert_eq!(out, bytes, "{label}: read without registries");
    assert_eq!(
        ParticleOptions::from_wire(unchecked, &block_lookup()).unwrap(),
        decoded
    );
    decoded
}

#[test]
fn particle_type_wire_ids_and_position_sources_match_the_golden() {
    for line in GOLDEN.lines().filter_map(|line| line.strip_prefix("type ")) {
        let mut parts = line.split(' ');
        let id: u16 = parts.next().unwrap().parse().unwrap();
        let name = parts.next().unwrap();
        assert_eq!(
            ParticleType::from_protocol_id(id).unwrap().as_static_str(),
            name
        );
    }

    let (_, sources) = mcrs_minecraft_registry_catalog::STATIC_REGISTRIES
        .iter()
        .find(|(key, _)| *key == "minecraft:position_source_type")
        .unwrap();
    let names: Vec<String> = sources.iter().map(ToString::to_string).collect();
    assert_eq!(names, ["minecraft:block", "minecraft:entity"]);
    let mut out = Vec::new();
    PositionSource::Entity {
        entity_id: 7,
        y_offset: 0.5,
    }
    .encode(&mut out)
    .unwrap();
    assert_eq!(out, [1, 7, 0x3f, 0, 0, 0]);
}

#[test]
fn every_golden_particle_matches_vanilla() {
    let labels: Vec<&str> = GOLDEN
        .lines()
        .filter_map(|line| line.split_once(".wire = "))
        .map(|(label, _)| label)
        .filter(|label| !label.starts_with("particles_packet"))
        .filter(|label| !label.ends_with("_display") && !label.starts_with("advancements"))
        .filter(|label| !["root", "seen_opened", "seen_closed"].contains(label))
        .collect();
    assert!(labels.len() >= 25, "{labels:?}");
    for label in labels {
        check(label);
    }
}

#[test]
fn golden_values_decode_to_the_expected_fields() {
    assert_eq!(check("angry_villager"), ParticleOptions::AngryVillager);
    assert_eq!(
        check("block"),
        ParticleOptions::Block(BlockParticle {
            block_state: BlockStateValue {
                block: ResourceKeyBuf::from_location(
                    mcrs_minecraft_resource::rl!("minecraft:furnace").to_buf()
                ),
                properties: BTreeMap::from([
                    ("facing".to_string(), "north".to_string()),
                    ("lit".to_string(), "true".to_string()),
                ]),
            },
        })
    );
    assert_eq!(
        check("block_furnace_default"),
        ParticleOptions::Block(BlockParticle {
            block_state: BlockStateValue {
                block: ResourceKeyBuf::from_location(
                    mcrs_minecraft_resource::rl!("minecraft:furnace").to_buf()
                ),
                properties: BTreeMap::new(),
            },
        })
    );
    assert_eq!(
        check("dust_scaled"),
        ParticleOptions::Dust(DustParticle {
            color: RgbInt(0x123456),
            scale: ParticleScale(2.25),
        })
    );
    assert_eq!(
        check("effect_default"),
        ParticleOptions::Effect(SpellParticle {
            color: RgbInt(-1),
            power: 1.0,
        })
    );
    assert_eq!(
        check("entity_effect"),
        ParticleOptions::EntityEffect(ColorParticle {
            color: ArgbInt(0x80FF8000u32 as i32),
        })
    );
    assert_eq!(
        check("item_stack"),
        ParticleOptions::Item(ItemParticle {
            item: Template::new(
                mcrs_minecraft_item::keys::Item::DiamondSword.id(),
                3,
                read_json::<ComponentPatch>(r#"{"max_stack_size":16}"#).unwrap(),
            )
            .unwrap(),
        })
    );
    assert_eq!(
        check("vibration"),
        ParticleOptions::Vibration(VibrationParticle {
            destination: PositionSource::Block {
                pos: BlockPos::new(10, -20, 30),
            },
            arrival_in_ticks: 40,
        })
    );
    assert_eq!(
        check("trail"),
        ParticleOptions::Trail(TrailParticle {
            target: [1.5, 2.5, -3.5],
            color: RgbInt(0xABCDEF),
            duration: Bounded(12),
        })
    );
}

#[test]
fn data_forms_validate_like_vanilla() {
    let error = |json: &str| read_json::<ParticleOptions>(json).unwrap_err().to_string();
    assert!(
        error(r#"{"type":"minecraft:dust","color":1,"scale":5.0}"#)
            .contains("Value must be within range [0.01;4.0]: 5.0")
    );
    assert!(
        error(r#"{"type":"minecraft:geyser","water_blocks":0}"#)
            .contains(golden("geyser_zero.parse_error"))
    );
    assert!(
        error(r#"{"type":"minecraft:trail","target":[0,0,0],"color":1,"duration":0}"#)
            .contains(golden("trail_zero.parse_error"))
    );
    assert!(error(r#"{"type":"minecraft:nope"}"#).contains("nope"));
    assert!(error(
        r#"{"type":"minecraft:vibration","destination":{"type":"minecraft:entity","source_entity":[1,2,3,4]},"arrival_in_ticks":1}"#
    )
    .contains("Entity position sources are not allowed"));

    let bare: ParticleOptions = read_json(r#"{"type":"flame"}"#).unwrap();
    assert_eq!(bare, ParticleOptions::Flame);
    let rgb: ParticleOptions =
        read_json(r#"{"type":"dust","color":[1.0,0.0,0.0],"scale":1}"#).unwrap();
    assert_eq!(to_hex(&wire(&rgb)), "15ffff00003f800000");
    let unknown: ParticleOptions =
        read_json(r#"{"type":"block","block_state":"minecraft:nope"}"#).unwrap();
    assert!(
        unknown
            .encode_checked(&block_lookup(), &mut Vec::new())
            .unwrap_err()
            .to_string()
            .contains("has no block state")
    );

    let clamped =
        ParticleOptions::decode_checked(&block_lookup(), &mut &hex("1500ff000041200000")[..])
            .unwrap();
    assert_eq!(
        clamped,
        ParticleOptions::Dust(DustParticle {
            color: RgbInt(0xFF0000),
            scale: ParticleScale(4.0),
        })
    );
}

#[test]
fn level_particles_packet_matches_vanilla() {
    let bytes = hex(golden("particles_packet.wire"));
    let mut r = &bytes[..];
    let packet = ClientboundLevelParticles::decode(&mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(
        packet,
        ClientboundLevelParticles {
            particle: ParticleOptions::decode(&mut &hex(golden("dust.wire"))[..]).unwrap(),
            override_limiter: true,
            always_show: false,
            pos: DVec3::new(1.5, 64.25, -3.0),
            dist: [0.5, 0.75, 1.0],
            max_speed: [0.1; 3],
            count: VarInt(25),
            randomization: ParticleRandomization::Default,
        }
    );
    assert_eq!(
        ParticleOptions::from_wire(packet.particle.clone(), &block_lookup()).unwrap(),
        check("dust")
    );
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));

    let bytes = hex(golden("particles_packet_item.wire"));
    let packet = ClientboundLevelParticles::decode(&mut &bytes[..]).unwrap();
    assert_eq!(packet.count, VarInt(3));
    assert_eq!(
        ParticleOptions::from_wire(packet.particle.clone(), &block_lookup()).unwrap(),
        check("item")
    );
    let rebuilt = ClientboundLevelParticles {
        particle: check("item").to_wire(&block_lookup()).unwrap(),
        ..packet
    };
    let mut out = Vec::new();
    rebuilt.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));

    for (id, randomization) in [
        (1, ParticleRandomization::Alternative),
        (2, ParticleRandomization::AlternativeWithSpeed),
        (3, ParticleRandomization::Default),
    ] {
        let mut bytes = bytes.clone();
        *bytes.last_mut().unwrap() = id;
        let packet = ClientboundLevelParticles::decode(&mut &bytes[..]).unwrap();
        assert_eq!(packet.randomization, randomization, "id {id}");
    }
}

#[test]
fn block_states_read_from_data_resolve_like_vanilla() {
    let labels: Vec<&str> = GOLDEN
        .lines()
        .filter_map(|line| line.split_once(".input = "))
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels.len(), 7, "{labels:?}");
    for label in labels {
        let parsed: ParticleOptions = read_json(golden(&format!("{label}.input"))).unwrap();
        let bytes = wire(&parsed);
        assert_eq!(
            to_hex(&bytes),
            golden(&format!("{label}.parsed_wire")),
            "{label}: wire"
        );
        let resolved = ParticleOptions::decode_checked(&block_lookup(), &mut &bytes[..]).unwrap();
        let json: serde_json::Value = serde_json::to_value(named(&resolved)).unwrap();
        let vanilla: serde_json::Value =
            serde_json::from_str(golden(&format!("{label}.parsed_json"))).unwrap();
        assert_eq!(json, vanilla, "{label}: json of the resolved state");
    }
}

#[test]
fn a_wire_value_that_names_no_known_entry_is_rejected_by_the_check() {
    use mcrs_minecraft_core::BlockStateNumber;
    use mcrs_minecraft_item::component::CompactList;
    use mcrs_minecraft_item::component::{AdventureModePredicate, BlockPredicate, CanPlaceOn};
    use mcrs_minecraft_item::keys::Item;
    use mcrs_minecraft_item::kind::ItemComponentValue;
    use mcrs_minecraft_item::stack::ItemStackValue;
    use mcrs_minecraft_registries::{WireId, WireSet};

    let item = |id: u16, components: ComponentPatch<Wire>| {
        ParticleOptions::<Wire>::Item(ItemParticle {
            item: Template(ItemStackValue {
                item: WireId::new(id),
                count: Bounded(1),
                components,
            }),
        })
    };
    let mut unknown_tag = ComponentPatch::<Wire>::default();
    unknown_tag.set_value(ItemComponentValue::CanPlaceOn(CanPlaceOn(
        AdventureModePredicate(CompactList(vec![BlockPredicate {
            blocks: Some(WireSet::Tag(TagKeyBuf::from_location(
                ResourceLocationBuf::read("mcrs:nope").unwrap(),
            ))),
            ..BlockPredicate::default()
        }])),
    )));
    let stone = Item::Stone.id().number();
    let cases = [
        (
            "unknown item id",
            item(u16::MAX, ComponentPatch::default()),
            "has no id 65535",
        ),
        ("unknown tag", item(stone, unknown_tag), "mcrs:nope"),
        (
            "unknown block state",
            ParticleOptions::Block(BlockParticle {
                block_state: BlockStateNumber(u16::MAX),
            }),
            "no block state has id 65535",
        ),
    ];
    for (label, wire, message) in cases {
        let mut bytes = Vec::new();
        wire.encode(&mut bytes).unwrap();
        let decoded = ParticleOptions::<Wire>::decode(&mut &bytes[..])
            .unwrap_or_else(|e| panic!("{label}: the wire form reads without registries: {e}"));
        let error = ParticleOptions::from_wire(decoded, &block_lookup()).unwrap_err();
        assert!(error.to_string().contains(message), "{label}: {error}");
    }
}
