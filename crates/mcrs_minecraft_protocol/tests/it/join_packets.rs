//! Bytes written by the vanilla packet stream codecs; `id` lines are the registry
//! ids the capture session had.

use crate::common::encoded;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation, VERSION};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::game_mode::OptGameMode;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::{Brand, clientbound, serverbound};
use mcrs_minecraft_protocol::packets::game::clientbound::*;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::{
    Bounded, Decode, Encode, GameMode, GlobalPos, LookAnchor, RawBytes, VarInt,
};

static GOLDEN: LazyLock<String> =
    LazyLock::new(|| mcrs_minecraft_fixtures::fixture("join_packets").text());

struct Fixture {
    ids: HashMap<(String, String), u16>,
    packets: HashMap<String, Vec<u8>>,
}

fn fixture() -> Fixture {
    let mut fixture = Fixture {
        ids: HashMap::new(),
        packets: HashMap::new(),
    };
    for line in GOLDEN.lines() {
        if let Some(rest) = line.strip_prefix("id ") {
            let [registry, name, id] = rest.split(' ').collect::<Vec<_>>()[..] else {
                panic!("malformed id line: {line}");
            };
            fixture
                .ids
                .insert((registry.into(), name.into()), id.parse().unwrap());
        } else if let Some((name, hex)) = line.split_once(' ') {
            fixture.packets.insert(name.into(), crate::common::hex(hex));
        }
    }
    fixture
}

fn dimension_type(fixture: &Fixture, name: &str) -> Id<DimensionType> {
    let number = fixture.ids[&("dimension_type".into(), name.into())];
    Id::checked(number, usize::from(u16::MAX) + 1).unwrap()
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
}

fn check<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(
    fixture: &'a Fixture,
    name: &str,
    expected: P,
) {
    let (packet, bytes) = decode::<P>(fixture, name);
    assert_eq!(packet, expected, "{name}");
    assert_eq!(encoded(&expected), bytes, "{name}");
}

fn overworld() -> ResourceKey<Dimension> {
    mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into()
}

#[test]
fn login_carries_no_seed_and_equals_the_reference_bytes() {
    let fixture = fixture();
    let expected = ClientboundLogin {
        player_id: 7,
        hardcore: false,
        dimensions: vec![overworld()],
        max_players: VarInt(20),
        chunk_radius: VarInt(10),
        simulation_distance: VarInt(8),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        player_spawn_info: PlayerSpawnInfo {
            dimension_type_id: dimension_type(&fixture, "minecraft:overworld"),
            dimension: overworld(),
            game_mode: GameMode::Creative,
            prev_game_mode: OptGameMode(Some(GameMode::Survival)),
            is_debug: false,
            is_flat: false,
            last_depth_location: None,
            portal_cooldown: VarInt(5),
            sea_level: VarInt(63),
        },
        online_mode: false,
        enforces_secure_chat: true,
    };
    check(&fixture, "login", expected);
}

#[test]
fn respawn_equals_the_reference_bytes() {
    let fixture = fixture();
    let expected = ClientboundRespawn {
        player_spawn_info: PlayerSpawnInfo {
            dimension_type_id: dimension_type(&fixture, "minecraft:the_nether"),
            dimension: mcrs_minecraft_dimension::keys::dimension::THE_NETHER.into(),
            game_mode: GameMode::Survival,
            prev_game_mode: OptGameMode(None),
            is_debug: false,
            is_flat: true,
            last_depth_location: Some(GlobalPos {
                dimension: overworld(),
                pos: BlockPos::new(1, 64, -3),
            }),
            portal_cooldown: VarInt(0),
            sea_level: VarInt(32),
        },
        data_to_keep: 3,
    };
    check(&fixture, "respawn", expected);
}

fn handshake<'a>(host: &'a str, intent: Intent) -> ServerboundHandshake<'a> {
    ServerboundHandshake {
        protocol_version: VarInt(VERSION.protocol_version),
        server_address: Bounded(host),
        server_port: 25565,
        intent,
    }
}

fn handshake_frame(host: &str) -> Vec<u8> {
    let mut frame = Vec::new();
    VarInt(VERSION.protocol_version).encode(&mut frame).unwrap();
    VarInt(host.len() as i32).encode(&mut frame).unwrap();
    frame.extend_from_slice(host.as_bytes());
    frame.extend_from_slice(&25565u16.to_be_bytes());
    VarInt(1).encode(&mut frame).unwrap();
    frame
}

fn decode_handshake(frame: &[u8]) -> mcrs_minecraft_protocol::Result<ServerboundHandshake<'_>> {
    let mut r = frame;
    ServerboundHandshake::decode(&mut r)
}

#[test]
fn the_handshake_equals_the_reference_bytes() {
    let fixture = fixture();
    let at_bound = "a".repeat(1024);
    check(
        &fixture,
        "intention",
        handshake("example.org", Intent::Login),
    );
    check(
        &fixture,
        "intention_host_at_bound",
        handshake(&at_bound, Intent::Status),
    );
}

#[test]
fn the_host_bound_counts_utf16_units() {
    let at_bound = "\u{1F600}".repeat(512);
    assert_eq!(at_bound.len(), 2048);
    let packet = handshake(&at_bound, Intent::Status);
    let bytes = encoded(&packet);
    assert_eq!(decode_handshake(&bytes).unwrap(), packet);

    let past = "\u{1F600}".repeat(513);
    assert!(
        handshake(&past, Intent::Status)
            .encode(&mut Vec::new())
            .is_err()
    );
    assert!(decode_handshake(&handshake_frame(&past)).is_err());
}

fn mod_name() -> ResourceLocation<Cow<'static, str>> {
    ResourceLocation::read_cow("github.com:andreypfau/mcrs").unwrap()
}

#[test]
fn a_brand_equals_the_reference_bytes() {
    let fixture = fixture();
    check(
        &fixture,
        "brand_serverbound",
        serverbound::Payload::Brand(Brand { brand: "sample" }),
    );
    check(
        &fixture,
        "brand_clientbound",
        clientbound::Payload::Brand(Brand { brand: "sample" }),
    );
}

#[test]
fn a_mod_list_equals_the_reference_bytes() {
    let fixture = fixture();
    let commit = ResourceLocation::read_cow("mcrs:commit").unwrap();
    check(
        &fixture,
        "mod_list",
        serverbound::Payload::ModList(serverbound::ModList(vec![(
            mod_name(),
            serverbound::PropertyMap(vec![(commit, "unknown")]),
        )])),
    );
    check(
        &fixture,
        "mod_list_empty",
        serverbound::Payload::ModList(serverbound::ModList(vec![])),
    );
    check(
        &fixture,
        "mod_list_entry_without_property",
        serverbound::Payload::ModList(serverbound::ModList(vec![(
            mod_name(),
            serverbound::PropertyMap(vec![]),
        )])),
    );
}

#[test]
fn an_unknown_channel_keeps_its_raw_bytes() {
    let body = [0u8, 255, 1, 2, 3];
    let mut frame = Vec::new();
    "example:data".encode(&mut frame).unwrap();
    frame.extend_from_slice(&body);
    let channel = || ResourceLocation::read_cow("example:data").unwrap();

    let mut r = &frame[..];
    let payload = serverbound::Payload::decode(&mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(
        payload,
        serverbound::Payload::Raw(serverbound::CustomPayload {
            channel: channel(),
            data: Bounded(RawBytes(&body)),
        })
    );
    assert_eq!(encoded(&payload), frame);

    let mut r = &frame[..];
    let payload = clientbound::Payload::decode(&mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(
        payload,
        clientbound::Payload::Raw(clientbound::CustomPayload {
            channel: channel(),
            data: Bounded(Cow::Owned(RawBytes(&body))),
        })
    );
    assert_eq!(encoded(&payload), frame);
}

#[test]
fn a_raw_payload_cannot_take_the_channel_of_a_typed_one() {
    let raw = |channel: &'static str| serverbound::CustomPayload {
        channel: ResourceLocation::read_cow(channel).unwrap(),
        data: Bounded(RawBytes(&[])),
    };
    for channel in ["minecraft:brand", "minecraft:mod_list"] {
        assert!(
            serverbound::Payload::Raw(raw(channel))
                .encode(&mut Vec::new())
                .is_err(),
            "{channel}"
        );
    }
}

#[test]
fn an_unknown_channel_body_is_bounded_by_the_direction() {
    let frame = |size: usize| {
        let mut frame = Vec::new();
        "example:data".encode(&mut frame).unwrap();
        frame.resize(frame.len() + size, 0);
        frame
    };
    assert!(serverbound::Payload::decode(&mut &frame(serverbound::MAX_PAYLOAD_SIZE)[..]).is_ok());
    assert!(
        serverbound::Payload::decode(&mut &frame(serverbound::MAX_PAYLOAD_SIZE + 1)[..]).is_err()
    );
    assert!(clientbound::Payload::decode(&mut &frame(clientbound::MAX_PAYLOAD_SIZE)[..]).is_ok());
    assert!(
        clientbound::Payload::decode(&mut &frame(clientbound::MAX_PAYLOAD_SIZE + 1)[..]).is_err()
    );
}

#[test]
fn abilities_and_look_at_follow_their_field_layouts() {
    let survival = ClientboundPlayerAbilities {
        invulnerable: false,
        flying: false,
        may_fly: false,
        instant_build: false,
        flying_speed: 0.05,
        walking_speed: 0.1,
    };
    let bytes = crate::common::hex("003d4ccccd3dcccccd");
    assert_eq!(encoded(&survival), bytes);
    let single_flags = [
        (
            0x01,
            ClientboundPlayerAbilities {
                invulnerable: true,
                ..survival
            },
        ),
        (
            0x02,
            ClientboundPlayerAbilities {
                flying: true,
                ..survival
            },
        ),
        (
            0x04,
            ClientboundPlayerAbilities {
                may_fly: true,
                ..survival
            },
        ),
        (
            0x08,
            ClientboundPlayerAbilities {
                instant_build: true,
                ..survival
            },
        ),
    ];
    for (flag, abilities) in single_flags {
        let mut bytes = bytes.clone();
        bytes[0] = flag;
        assert_eq!(encoded(&abilities), bytes, "{flag:#04x}");
        assert_eq!(
            ClientboundPlayerAbilities::decode(&mut &bytes[..]).unwrap(),
            abilities,
            "{flag:#04x}"
        );
    }

    let at_cow = ClientboundPlayerLookAt {
        from: LookAnchor::Feet,
        target: glam::DVec3::new(1.0, 2.0, 3.0),
        entity: Some(LookAtEntity {
            id: VarInt(5),
            anchor: LookAnchor::Eyes,
        }),
    };
    let mut expected = vec![0];
    for value in [1.0f64, 2.0, 3.0] {
        expected.extend(value.to_be_bytes());
    }
    expected.extend([1, 5, 1]);
    assert_eq!(encoded(&at_cow), expected);
    let at_point = ClientboundPlayerLookAt {
        from: LookAnchor::Eyes,
        entity: None,
        ..at_cow
    };
    assert_eq!(encoded(&at_point)[0], 1);
    assert_eq!(encoded(&at_point).last(), Some(&0));
}
