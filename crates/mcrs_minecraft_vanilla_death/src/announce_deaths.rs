use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_engine::bus;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::PacketTarget;
use mcrs_minecraft_engine_player_vitals::DeathMessage;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_text::RawText;

pub(crate) fn announce_deaths(
    mut described: MessageReader<DeathMessage>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for message in described.read() {
        let Ok(content) = RawText::from_text(&message.text) else {
            continue;
        };
        writer.write(bus::to_target(
            PacketTarget::AllPlayers,
            PacketPayload::SystemChat(ClientboundSystemChatPacket {
                content,
                overlay: false,
            }),
        ));
    }
}
