use std::collections::BTreeMap;

use bevy_math::DVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_protocol::item::{
    ArgbInt, ComponentPatch, DecodeCtx, EncodeCtx, RgbInt, Template,
};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLevelParticles;
use mcrs_minecraft_protocol::particle::{
    BlockParticle, BlockStateValue, ColorParticle, DustParticle, ItemParticle, ParticleKind,
    ParticleOptions, ParticleScale, PositionSource, RawParticle, SpellParticle, TrailParticle,
    VibrationParticle,
};
use mcrs_minecraft_protocol::{Decode, Encode};

use crate::harness::{TestLookup, hex};

const GOLDEN: &str = include_str!("../fixtures/particles_golden.txt");

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

pub(crate) fn lookup() -> TestLookup {
    let mut lookup = TestLookup::new();
    lookup.registry_with_ids(
        "item",
        &[
            ("air", 0),
            ("stone", 1),
            ("apple", 1007),
            ("diamond_sword", 1050),
        ],
    );
    lookup.block_state(1, "stone", &[], true);
    lookup.block_state(
        6884,
        "furnace",
        &[("facing", "north"), ("lit", "false")],
        true,
    );
    lookup.block_state(
        6883,
        "furnace",
        &[("facing", "north"), ("lit", "true")],
        false,
    );
    lookup.block_state(
        6886,
        "furnace",
        &[("facing", "south"), ("lit", "false")],
        false,
    );
    lookup
}

fn wire(particle: &ParticleOptions) -> Vec<u8> {
    let mut out = Vec::new();
    particle.encode_ctx(&lookup(), &mut out).unwrap();
    out
}

/// Decodes the golden wire bytes, checks they re-encode byte for byte, and
/// that the golden JSON parses to the same value and is written back as it
/// was read.
fn check(label: &str) -> ParticleOptions {
    let bytes = hex(golden(&format!("{label}.wire")));
    let mut r = &bytes[..];
    let decoded = ParticleOptions::decode_ctx(&lookup(), &mut r).unwrap();
    assert!(r.is_empty(), "{label}: {} trailing bytes", r.len());
    assert_eq!(to_hex(&wire(&decoded)), to_hex(&bytes), "{label}: wire");

    let json = golden(&format!("{label}.json"));
    let parsed: ParticleOptions = serde_json::from_str(json).unwrap();
    assert_eq!(parsed, decoded, "{label}: json {json}");
    let back: serde_json::Value = serde_json::to_value(&decoded).unwrap();
    let vanilla: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(back, vanilla, "{label}: json round trip");

    let raw = RawParticle::decode(&mut &bytes[..]).unwrap();
    assert_eq!(raw.0, bytes, "{label}: raw walk");
    assert_eq!(raw.resolve(&lookup()).unwrap(), decoded);
    decoded
}

#[test]
fn particle_kinds_match_the_registry_report() {
    let report: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../assets/mcrs/reports/registries.json"
    ))
    .unwrap();
    let entries = report["minecraft:particle_type"]["entries"]
        .as_object()
        .unwrap();
    assert_eq!(entries.len(), ParticleKind::COUNT);
    for (name, entry) in entries {
        let kind = ParticleKind::from_id(name).unwrap_or_else(|| panic!("{name} is not modelled"));
        assert_eq!(
            kind as i64,
            entry["protocol_id"].as_i64().unwrap(),
            "{name}"
        );
        assert_eq!(kind.id().as_str(), name);
        assert_eq!(ParticleKind::from_wire_id(kind as i32), Some(kind));
    }
    for line in GOLDEN.lines().filter_map(|line| line.strip_prefix("type ")) {
        let mut parts = line.split(' ');
        let id: i32 = parts.next().unwrap().parse().unwrap();
        let name = parts.next().unwrap();
        assert_eq!(ParticleKind::from_wire_id(id).unwrap().id().as_str(), name);
    }

    let sources = report["minecraft:position_source_type"]["entries"]
        .as_object()
        .unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources["minecraft:block"]["protocol_id"], 0);
    assert_eq!(sources["minecraft:entity"]["protocol_id"], 1);
    let mut out = Vec::new();
    PositionSource::Entity {
        entity_id: mcrs_minecraft_protocol::VarInt(7),
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
                block: ResourceKey::from_location(
                    mcrs_minecraft_core::ResourceLocation::minecraft("furnace")
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
                block: ResourceKey::from_location(
                    mcrs_minecraft_core::ResourceLocation::minecraft("furnace")
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
                ResourceKey::from_location(mcrs_minecraft_core::ResourceLocation::minecraft(
                    "diamond_sword"
                )),
                3,
                serde_json::from_str::<ComponentPatch>(r#"{"max_stack_size":16}"#).unwrap(),
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
    let error = |json: &str| {
        serde_json::from_str::<ParticleOptions>(json)
            .unwrap_err()
            .to_string()
    };
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

    let bare: ParticleOptions = serde_json::from_str(r#"{"type":"flame"}"#).unwrap();
    assert_eq!(bare, ParticleOptions::Flame);
    let rgb: ParticleOptions =
        serde_json::from_str(r#"{"type":"dust","color":[1.0,0.0,0.0],"scale":1}"#).unwrap();
    assert_eq!(to_hex(&wire(&rgb)), "15ffff00003f800000");
    let unknown: ParticleOptions =
        serde_json::from_str(r#"{"type":"block","block_state":"minecraft:nope"}"#).unwrap();
    assert!(
        unknown
            .encode_ctx(&lookup(), &mut Vec::new())
            .unwrap_err()
            .to_string()
            .contains("has no block state")
    );

    let clamped =
        ParticleOptions::decode_ctx(&lookup(), &mut &hex("1500ff000041200000")[..]).unwrap();
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
            override_limiter: true,
            always_show: false,
            pos: DVec3::new(1.5, 64.25, -3.0),
            dist: [0.5, 0.75, 1.0],
            max_speed: 0.1,
            count: 25,
            particle: RawParticle(hex(golden("dust.wire")).into()),
        }
    );
    assert_eq!(packet.particle.resolve(&lookup()).unwrap(), check("dust"));
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));

    let bytes = hex(golden("particles_packet_item.wire"));
    let packet = ClientboundLevelParticles::decode(&mut &bytes[..]).unwrap();
    assert_eq!(packet.count, 3);
    assert_eq!(packet.particle.resolve(&lookup()).unwrap(), check("item"));
    let rebuilt = ClientboundLevelParticles {
        particle: RawParticle::from_options(&check("item"), &lookup()).unwrap(),
        ..packet
    };
    let mut out = Vec::new();
    rebuilt.encode(&mut out).unwrap();
    assert_eq!(to_hex(&out), to_hex(&bytes));
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
        let parsed: ParticleOptions =
            serde_json::from_str(golden(&format!("{label}.input"))).unwrap();
        let bytes = wire(&parsed);
        assert_eq!(
            to_hex(&bytes),
            golden(&format!("{label}.parsed_wire")),
            "{label}: wire"
        );
        let resolved = ParticleOptions::decode_ctx(&lookup(), &mut &bytes[..]).unwrap();
        let json: serde_json::Value = serde_json::to_value(&resolved).unwrap();
        let vanilla: serde_json::Value =
            serde_json::from_str(golden(&format!("{label}.parsed_json"))).unwrap();
        assert_eq!(json, vanilla, "{label}: json of the resolved state");
    }
}
