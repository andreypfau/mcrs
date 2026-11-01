use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_text::{RawText, Text};

/// A system chat message to the player with the host anchor `host`, or `None` when `content`
/// cannot be encoded.
pub fn system_message(host: Entity, content: &Text) -> Option<OutboundPlayerPacket> {
    let content = RawText::from_text(content).ok()?;
    Some(bus::to(host, system_chat(content)))
}

/// The payload of a system chat message, outside the action bar.
pub fn system_chat(content: RawText<'static>) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}
