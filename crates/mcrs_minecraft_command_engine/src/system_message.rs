use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::text::Text;

pub fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
    to(host, system_chat(content))
}

pub fn system_chat(content: Text) -> PacketPayload {
    PacketPayload::SystemChat(ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
}
