//! The handshake and the custom payloads, against the bytes the game writes for them. They share
//! the join packets' golden file with the play packets the game crate checks.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::{Brand, clientbound, serverbound};
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, RawBytes, VarInt};
use mcrs_minecraft_resource::ResourceLocation;

use crate::encoded;

static GOLDEN: LazyLock<HashMap<String, Vec<u8>>> = LazyLock::new(|| {
    mcrs_minecraft_fixtures::fixture("join_packets")
        .text()
        .lines()
        .filter(|line| !line.starts_with("id "))
        .filter_map(|line| line.split_once(' '))
        .map(|(name, hex)| (name.to_owned(), crate::hex(hex)))
        .collect()
});

fn check<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(name: &str, expected: P) {
    let bytes: &'a [u8] = &GOLDEN[name];
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap_or_else(|e| panic!("{name}: {e:#}"));
    assert!(r.is_empty(), "{name}: {} trailing bytes", r.len());
    assert_eq!(packet, expected, "{name}");
    assert_eq!(encoded(&expected), bytes, "{name}");
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
    let at_bound = "a".repeat(1024);
    check("intention", handshake("example.org", Intent::Login));
    check(
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

fn mod_name() -> ResourceLocation<'static> {
    ResourceLocation::read("github.com:andreypfau/mcrs").unwrap()
}

#[test]
fn a_brand_equals_the_reference_bytes() {
    check(
        "brand_serverbound",
        serverbound::Payload::Brand(Brand { brand: "sample" }),
    );
    check(
        "brand_clientbound",
        clientbound::Payload::Brand(Brand { brand: "sample" }),
    );
}

#[test]
fn a_mod_list_equals_the_reference_bytes() {
    let commit = ResourceLocation::read("mcrs:commit").unwrap();
    check(
        "mod_list",
        serverbound::Payload::ModList(serverbound::ModList(vec![(
            mod_name(),
            serverbound::PropertyMap(vec![(commit, "unknown")]),
        )])),
    );
    check(
        "mod_list_empty",
        serverbound::Payload::ModList(serverbound::ModList(vec![])),
    );
    check(
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
    let channel = || ResourceLocation::read("example:data").unwrap();

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
        channel: ResourceLocation::read(channel).unwrap(),
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
