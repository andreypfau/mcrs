use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::text::{self, Text};
use mcrs_minecraft_text::Text as PlainText;

/// A system chat message to the player with the host anchor `host`, or `None` when `content`
/// names an item, an entity or a dialog, which no command emits.
pub fn system_message(host: Entity, content: &PlainText) -> Option<OutboundPlayerPacket> {
    let content = text::named(content).ok()?;
    Some(bus::to(host, system_chat(content)))
}

/// The payload of a system chat message, outside the action bar.
pub fn system_chat(content: Text) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}
