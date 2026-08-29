//! Every `Decode` implementation must consume exactly the bytes it read, so a
//! field can be followed by another one. NBT compounds, registry holders and
//! chat components each used to leave the reader where they found it, or to
//! swallow the whole remaining buffer.

use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundRegistryData;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::registry::Holder;
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::compound::NbtCompound;

fn compound(name: &str, value: i32) -> NbtCompound {
    let mut c = NbtCompound::new();
    c.put_int(name, value);
    c
}

fn encoded(value: &impl Encode) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf).expect("encode");
    buf
}

#[test]
fn nbt_compound_leaves_the_reader_past_its_own_bytes() {
    let first = compound("first", 1);
    let mut buf = encoded(&first);
    let tail_offset = buf.len();
    VarInt(0x2A).encode(&mut buf).expect("encode tail");

    let mut r: &[u8] = &buf;
    assert_eq!(NbtCompound::decode(&mut r).expect("decode compound"), first);
    assert_eq!(
        r.len(),
        buf.len() - tail_offset,
        "the compound did not advance the reader by its own length"
    );
    assert_eq!(VarInt::decode(&mut r).expect("decode tail").0, 0x2A);
    assert!(r.is_empty());
}

#[test]
fn registry_data_with_entries_round_trips() {
    let packet = ClientboundRegistryData {
        registry: ResourceLocation::parse_cow("minecraft:dimension_type").expect("registry id"),
        entries: vec![
            mcrs_minecraft_protocol::registry::Entry {
                id: ResourceLocation::parse_cow("minecraft:overworld").expect("entry id"),
                data: Some(std::borrow::Cow::Owned(compound("height", 384))),
            },
            mcrs_minecraft_protocol::registry::Entry {
                id: ResourceLocation::parse_cow("minecraft:the_nether").expect("entry id"),
                data: Some(std::borrow::Cow::Owned(compound("height", 256))),
            },
        ],
    };

    let buf = encoded(&packet);
    let mut r: &[u8] = &buf;
    let decoded = ClientboundRegistryData::decode(&mut r).expect("decode registry data");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    assert_eq!(encoded(&decoded), buf);
    assert_eq!(decoded.entries.len(), 2);
}

#[test]
fn direct_holder_leaves_the_reader_past_its_own_bytes() {
    let holder = Holder::Direct(compound("value", 7));
    let mut buf = encoded(&holder);
    VarInt(0x2A).encode(&mut buf).expect("encode tail");

    let mut r: &[u8] = &buf;
    match Holder::decode(&mut r).expect("decode holder") {
        Holder::Direct(c) => assert_eq!(c, compound("value", 7)),
        other => panic!("expected a direct holder, got {other:?}"),
    }
    assert_eq!(VarInt::decode(&mut r).expect("decode tail").0, 0x2A);
    assert!(r.is_empty());
}

#[test]
fn reference_holder_round_trips() {
    let mut buf = encoded(&Holder::Reference(12));
    VarInt(0x2A).encode(&mut buf).expect("encode tail");

    let mut r: &[u8] = &buf;
    match Holder::decode(&mut r).expect("decode holder") {
        Holder::Reference(id) => assert_eq!(id, 12),
        other => panic!("expected a reference holder, got {other:?}"),
    }
    assert_eq!(VarInt::decode(&mut r).expect("decode tail").0, 0x2A);
    assert!(r.is_empty());
}

#[test]
fn text_is_not_the_last_field_of_a_system_chat_packet() {
    let content = Text::text("hello");
    let packet = ClientboundSystemChatPacket {
        content: content.clone(),
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
    assert_eq!(decoded.content, content);
    assert!(decoded.overlay, "overlay flag was not decoded");
    assert_eq!(encoded(&decoded), buf);
}
