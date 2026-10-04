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

#[test]
fn light_updates_encode_to_the_fixture_bytes_and_decode_back() {
    let one_section = LightData {
        sky_light_mask: Cow::Owned(vec![1u64]),
        block_light_mask: Cow::Borrowed(&[]),
        empty_sky_light_mask: Cow::Borrowed(&[]),
        empty_block_light_mask: Cow::Owned(vec![1u64]),
        sky_light_arrays: Cow::Owned(vec![LightChunk([0xFFu8; 2048])]),
        block_light_arrays: Cow::Borrowed(&[]),
    };
    for (light_data, fixture) in [
        (LightData::default(), EMPTY_FIXTURE),
        (one_section, ONE_SECTION_FIXTURE),
    ] {
        let packet = ClientboundLightUpdate {
            x: VarInt(0),
            z: VarInt(0),
            light_data,
        };
        let mut payload = Vec::new();
        packet.encode(&mut payload).expect("encode payload");
        assert_eq!(payload, fixture, "{packet:?}");

        let mut r: &[u8] = &payload;
        let decoded = ClientboundLightUpdate::decode(&mut r).expect("decode payload");
        assert!(r.is_empty(), "trailing bytes after decode");
        assert_eq!(decoded.x, packet.x);
        assert_eq!(decoded.z, packet.z);
        assert_eq!(decoded.light_data, packet.light_data);
    }
}
