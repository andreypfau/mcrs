use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_text::{RawText, Text};

/// A system chat message to the player of `session`, or `None` when `content` cannot be
/// encoded.
pub fn system_message(session: PlayerSession, content: &Text) -> Option<OutboundPlayerPacket> {
    let content = RawText::from_text(content).ok()?;
    Some(bus::to(session, system_chat(content)))
}

/// The payload of a system chat message, outside the action bar.
pub fn system_chat(content: RawText<'static>) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}
