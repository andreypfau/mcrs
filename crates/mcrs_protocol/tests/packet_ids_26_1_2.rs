//! Fixtures in `fixtures/clientbound_light_update_*.bin` contain the packet PAYLOAD ONLY
//! (no leading VarInt(packet_id) prefix). Tests that need framed-wire round-trips prepend
//! the packet-id themselves.

use mcrs_protocol::packets::game::clientbound::ClientboundLightUpdate;
use mcrs_protocol::{Decode, Encode, LightData, Packet, VarInt};
use std::borrow::Cow;

const EMPTY_FIXTURE: &[u8] = include_bytes!("fixtures/clientbound_light_update_empty.bin");
const ONE_SECTION_FIXTURE: &[u8] =
    include_bytes!("fixtures/clientbound_light_update_one_section.bin");
const CAPTURED_FIXTURE: &[u8] =
    include_bytes!("fixtures/clientbound_light_update_captured_26_1_2.bin");

#[test]
fn fixture_files_exist() {
    assert!(!EMPTY_FIXTURE.is_empty(), "empty-layout fixture is zero-length");
    assert!(
        !ONE_SECTION_FIXTURE.is_empty(),
        "one-section-layout fixture is zero-length"
    );
    assert!(
        !CAPTURED_FIXTURE.is_empty(),
        "captured-fixture placeholder is zero-length"
    );
}

/// Encodes only the packet body (no leading VarInt packet-id, no outer length
/// prefix), matching the pinned fixture-framing convention.
fn encode_payload<P: Encode>(pkt: &P) -> Vec<u8> {
    let mut buf = Vec::new();
    pkt.encode(&mut buf).expect("encode payload");
    buf
}

fn popcount(mask: &[u64]) -> u32 {
    mask.iter().map(|w| w.count_ones()).sum()
}

#[test]
fn clientbound_light_update_empty_round_trip() {
    let pkt = ClientboundLightUpdate {
        x: VarInt(0),
        z: VarInt(0),
        light_data: LightData::default(),
    };

    let payload = encode_payload(&pkt);
    assert_eq!(
        payload, EMPTY_FIXTURE,
        "encoded empty-layout payload must equal the hand-crafted fixture bytes"
    );

    let mut r: &[u8] = &payload;
    let decoded = ClientboundLightUpdate::decode(&mut r).expect("decode empty payload");
    assert!(r.is_empty(), "trailing bytes after decode");
    assert_eq!(decoded.x.0, 0);
    assert_eq!(decoded.z.0, 0);
    assert_eq!(decoded.light_data, LightData::default());

    // Verify the framed-wire round-trip too: prepend VarInt(packet_id) and
    // confirm the body parses identically.
    let mut framed = Vec::new();
    VarInt(ClientboundLightUpdate::ID)
        .encode(&mut framed)
        .expect("encode id");
    framed.extend_from_slice(&payload);
    let mut r: &[u8] = &framed[VarInt(ClientboundLightUpdate::ID).written_size()..];
    let _refetched = ClientboundLightUpdate::decode(&mut r).expect("decode framed");
}

#[test]
fn clientbound_light_update_one_section_round_trip() {
    let pkt = ClientboundLightUpdate {
        x: VarInt(0),
        z: VarInt(0),
        light_data: LightData {
            sky_light_mask: Cow::Owned(vec![1u64]),
            block_light_mask: Cow::Borrowed(&[]),
            empty_sky_light_mask: Cow::Borrowed(&[]),
            empty_block_light_mask: Cow::Owned(vec![1u64]),
            sky_light_arrays: Cow::Owned(vec![[0xFFu8; 2048]]),
            block_light_arrays: Cow::Borrowed(&[]),
        },
    };

    let payload = encode_payload(&pkt);
    assert_eq!(
        payload, ONE_SECTION_FIXTURE,
        "encoded one-section payload must equal the hand-crafted fixture bytes"
    );

    let mut r: &[u8] = &payload;
    let decoded = ClientboundLightUpdate::decode(&mut r).expect("decode one-section payload");
    assert!(r.is_empty(), "trailing bytes after decode");
    assert_eq!(decoded.x.0, 0);
    assert_eq!(decoded.z.0, 0);
    assert_eq!(
        decoded.light_data.sky_light_arrays.len() as u32,
        popcount(&decoded.light_data.sky_light_mask),
        "sky_light_arrays count must equal popcount(sky_light_mask)"
    );
    assert_eq!(
        decoded.light_data.sky_light_arrays.len(),
        1,
        "exactly one populated sky-light section"
    );
    assert_eq!(
        decoded.light_data.block_light_arrays.len(),
        0,
        "no populated block-light sections"
    );
    assert_eq!(
        &decoded.light_data.sky_light_arrays[0][..],
        &[0xFFu8; 2048][..],
        "populated sky-light section bytes must round-trip exactly"
    );
}

#[test]
#[ignore = "captures real bytes via packet capture during manual handshake; remove #[ignore] when the fixture is populated."]
fn clientbound_light_update_captured_fixture_round_trip() {
    let mut r: &[u8] = CAPTURED_FIXTURE;
    let decoded =
        ClientboundLightUpdate::decode(&mut r).expect("decode captured-fixture payload");
    assert!(
        r.is_empty(),
        "captured fixture has {} trailing bytes after decode",
        r.len()
    );
    let re_encoded = encode_payload(&decoded);
    assert_eq!(
        re_encoded, CAPTURED_FIXTURE,
        "captured fixture must round-trip byte-equal"
    );
}
