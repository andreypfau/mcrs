//! Every `Decode` implementation must consume exactly the bytes it read, so a
//! field can be followed by another one. NBT compounds, registry holders and
//! chat components each used to leave the reader where they found it, or to
//! swallow the whole remaining buffer.

use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol_game::text::{RawText, Text};

fn encoded(value: &impl Encode) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf).expect("encode");
    buf
}

#[test]
fn every_decoder_stops_after_its_own_bytes() {
    {
        let content = Text::text("hello");
        let packet = ClientboundSystemChatPacket {
            content: RawText::from_text(&content).unwrap(),
            overlay: true,
        };

        let buf = encoded(&packet);
        let mut r: &[u8] = &buf;
        let decoded = ClientboundSystemChatPacket::decode(&mut r).expect("decode system chat");
        assert!(
            r.is_empty(),
            "the text field swallowed the overlay flag ({} bytes left)",
            r.len()
        );
        assert_eq!(decoded.content, packet.content);
        assert!(decoded.overlay, "overlay flag was not decoded");
        assert_eq!(encoded(&decoded), buf);
    }
}
