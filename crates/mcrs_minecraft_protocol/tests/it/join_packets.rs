//! Bytes written by the vanilla packet stream codecs; `id` lines are the registry
//! ids the capture session had.

use std::borrow::Cow;
use std::collections::HashMap;

use mcrs_minecraft_core::{BlockPos, ResourceLocation, VERSION};
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::game_mode::OptGameMode;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::{Brand, clientbound, serverbound};
use mcrs_minecraft_protocol::packets::game::clientbound::*;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, GameMode, GlobalPos, RawBytes, VarInt};

const GOLDEN: &str = include_str!("../fixtures/join_packets_golden.txt");

struct Fixture {
    ids: HashMap<(String, String), u32>,
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

fn id(fixture: &Fixture, registry: &str, name: &str) -> i32 {
    fixture.ids[&(registry.into(), name.into())] as i32
}

fn decode<'a, P: Decode<'a>>(fixture: &'a Fixture, name: &str) -> (P, &'a [u8]) {
    let bytes = &fixture.packets[name][..];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    (packet, bytes)
}

fn encoded<P: Encode>(packet: &P) -> Vec<u8> {
    let mut out = Vec::new();
    packet.encode(&mut out).unwrap();
    out
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

fn overworld() -> ResourceLocation<Cow<'static, str>> {
    ResourceLocation::from(mcrs_minecraft_core::rl!("minecraft:overworld"))
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
            dimension_type_id: VarInt(id(&fixture, "dimension_type", "minecraft:overworld")),
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
            dimension_type_id: VarInt(id(&fixture, "dimension_type", "minecraft:the_nether")),
            dimension: ResourceLocation::from(mcrs_minecraft_core::rl!("minecraft:the_nether")),
            game_mode: GameMode::Survival,
            prev_game_mode: OptGameMode(None),
            is_debug: false,
            is_flat: true,
            last_depth_location: Some(GlobalPos {
                dimension_name: overworld(),
                position: BlockPos::new(1, 64, -3),
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

fn decode_handshake(frame: &[u8]) -> anyhow::Result<ServerboundHandshake<'_>> {
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
    ResourceLocation::parse_cow("github.com:andreypfau/mcrs").unwrap()
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
    let commit = ResourceLocation::parse_cow("mcrs:commit").unwrap();
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
    let channel = || ResourceLocation::parse_cow("example:data").unwrap();

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
        channel: ResourceLocation::parse_cow(channel).unwrap(),
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
