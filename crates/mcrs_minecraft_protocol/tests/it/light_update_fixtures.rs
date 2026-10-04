//! Fixtures in `fixtures/clientbound_light_update_*.bin` contain the packet PAYLOAD ONLY
//! (no leading VarInt(packet_id) prefix). Tests that need framed-wire round-trips prepend
//! the packet-id themselves.

use mcrs_minecraft_protocol::chunk::LightChunk;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLightUpdate;
use mcrs_minecraft_protocol::{Decode, Encode, LightData, VarInt};
use std::borrow::Cow;

const EMPTY_FIXTURE: &[u8] = include_bytes!("../fixtures/clientbound_light_update_empty.bin");
const ONE_SECTION_FIXTURE: &[u8] =
    include_bytes!("../fixtures/clientbound_light_update_one_section.bin");

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
            sky_light_arrays: Cow::Owned(vec![LightChunk([0xFFu8; 2048])]),
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
        &decoded.light_data.sky_light_arrays[0].0[..],
        &[0xFFu8; 2048][..],
        "populated sky-light section bytes must round-trip exactly"
    );
}
