use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::entity::player::MessageSignature;
use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChat, ServerboundClientInformation,
};
use mcrs_minecraft_protocol::setting::ChatMode;
use mcrs_minecraft_protocol::setting::{DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::text::{IntoText, Text, TextContent};
use mcrs_minecraft_protocol::uuid::Uuid;

use crate::host_app::{self, PlayServer};

pub fn line(message: &str) -> ServerboundChat<'_> {
    ServerboundChat {
        message: Bounded(message),
        timestamp: 0,
        salt: 0,
        signature: None,
        last_seen_messages: MessageSignature {
            offset: VarInt(0),
            acknowledged: [0; 3],
            checksum: 0,
        },
    }
}

pub fn client_information(chat_mode: ChatMode) -> ServerboundClientInformation<'static> {
    ServerboundClientInformation(ClientInformation {
        locale: "en_us",
        view_distance: 2,
        chat_mode,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    })
}

fn key(text: &Text) -> Option<&str> {
    match &text.content {
        TextContent::Translate { translate, .. } => Some(translate),
        _ => None,
    }
}

#[test]
fn a_chat_line_reaches_everyone_unless_it_is_rejected_or_the_sender_hides_chat() {
    let cases = [
        ("a plain line", ChatMode::Enabled, "hello", true, None),
        (
            "an illegal character",
            ChatMode::Enabled,
            "a\u{a7}b",
            false,
            None,
        ),
        (
            "a control character",
            ChatMode::Enabled,
            "a\u{7}b",
            false,
            None,
        ),
        (
            "hidden chat",
            ChatMode::Hidden,
            "hello",
            false,
            Some("chat.disabled.options"),
        ),
    ];
    for (case, mode, message, broadcast, reply) in cases {
        let mut server = PlayServer::start(host_app::make_host_app(), &[], 0);
        let (sender, _) = server.join(Uuid::from_u128(1), "Sender");
        server.queue(sender, &client_information(mode));
        server.queue(sender, &line(message));
        let sent = server.ticks_all(2);

        let heard: Vec<&Text> = sent
            .iter()
            .filter(|packet| matches!(packet.target, PacketTarget::AllPlayers))
            .filter_map(|packet| match &packet.data {
                PacketPayload::SystemChat(chat) => Some(&chat.content),
                _ => None,
            })
            .collect();
        if broadcast {
            let expected = Text::translate(
                "chat.type.text",
                vec![
                    "Sender".to_owned().into_text(),
                    message.to_owned().into_text(),
                ],
            );
            assert_eq!(heard, [&expected], "{case}");
        } else {
            assert!(heard.is_empty(), "{case}: {heard:?}");
        }

        let told: Vec<&str> = sent
            .iter()
            .filter(|packet| matches!(packet.target, PacketTarget::SinglePlayer(target) if target == sender))
            .filter_map(|packet| match &packet.data {
                PacketPayload::SystemChat(chat) => key(&chat.content),
                _ => None,
            })
            .collect();
        assert_eq!(told, reply.into_iter().collect::<Vec<_>>(), "{case}");
    }
}
