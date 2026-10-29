use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::text::Text;

/// A system chat message to the player with the host anchor `host`.
pub fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
    bus::to(host, system_chat(content))
}

/// The payload of a system chat message, outside the action bar.
pub fn system_chat(content: Text) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}
