use mcrs_minecraft_protocol::chat::{ChatTypeId, FilterMask};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundDisguisedChat, ClientboundPlayerChat,
};
use mcrs_minecraft_protocol::text::RawText;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, VarInt};
use mcrs_minecraft_text::Text;
use mcrs_test_alloc::{CountingAllocator, allocations};
use uuid::Uuid;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn raw(json: &str) -> RawText<'static> {
    RawText::from_text(&serde_json::from_str::<Text>(json).unwrap()).unwrap()
}

fn encoded(packet: &impl Encode) -> Vec<u8> {
    let mut bytes = Vec::new();
    packet.encode(&mut bytes).unwrap();
    bytes
}

fn forwards_without_allocating<'a, P: Decode<'a> + Encode>(name: &str, bytes: &'a [u8]) {
    let mut out = Vec::with_capacity(bytes.len());
    let before = allocations();
    let mut r = bytes;
    let packet = P::decode(&mut r).unwrap();
    packet.encode(&mut out).unwrap();
    assert_eq!(allocations() - before, 0, "forwarding {name} allocates");
    assert!(r.is_empty(), "{name}: {} bytes left", r.len());
    assert_eq!(out, bytes, "{name}");
}

#[test]
fn a_chat_packet_forwards_its_text_without_allocating() {
    let sender = raw(
        r#"{"text": "Steve", "hover_event": {"action": "show_entity", "id": "player",
            "uuid": "00000000-0000-0000-0000-000000000001", "name": "Steve"}}"#,
    );
    let message = raw(
        r#"["look: ", {"text": "[Wand]", "color": "gold", "hover_event": {"action": "show_item",
            "id": "stick", "count": 3, "components": {"custom_name": "Wand", "!lore": {}}}}]"#,
    );
    let target = raw(r#"{"translate": "chat.type.team.text", "with": ["Blue"]}"#);

    let disguised = encoded(&ClientboundDisguisedChat {
        message: message.clone(),
        chat_type: ChatTypeId(0),
        sender_name: sender.clone(),
        target_name: Some(target.clone()),
    });
    forwards_without_allocating::<ClientboundDisguisedChat<'_>>("disguised_chat", &disguised);

    let player = encoded(&ClientboundPlayerChat {
        global_index: VarInt(0),
        sender: Uuid::from_u128(1),
        index: VarInt(0),
        signature: None,
        content: Bounded("look: [Wand]"),
        timestamp: 0,
        salt: 0,
        last_seen: Bounded(Vec::new()),
        unsigned_content: Some(message),
        filter: FilterMask::PassThrough,
        chat_type: ChatTypeId(0),
        sender_name: sender,
        target_name: Some(target),
    });
    forwards_without_allocating::<ClientboundPlayerChat<'_>>("player_chat", &player);
}

#[test]
fn a_raw_text_is_one_text_tag() {
    let text = raw(r#"{"text": "x"}"#);
    let mut bytes = encoded(&text);
    bytes.push(0x2a);
    let mut r = &bytes[..];
    assert_eq!(RawText::decode(&mut r).unwrap(), text);
    assert_eq!(r, [0x2a]);

    for bad in [
        &[][..],
        &[0x00],
        &[0x03, 0, 0, 0, 1],
        &[0x0a, 0x08, 0x00, 0x04, b't'],
    ] {
        assert!(RawText::decode(&mut &bad[..]).is_err(), "{bad:?}");
    }
}
