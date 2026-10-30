use bevy_app::{App, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::*;
use mcrs_command::is_allowed_chat_character;
use mcrs_minecraft_command_engine::system_chat;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketTarget, to_target};
use mcrs_minecraft_engine::client_info::{ClientInfo, ClientInfoPlugin};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_engine::session::HostAnchor;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::Text;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChat;
use mcrs_minecraft_protocol::setting::ChatMode;
use mcrs_minecraft_protocol::text::{Color, IntoText};
use tracing::info;

/// Broadcasts the chat messages of the players as system messages.
pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_observer(handle_chat);
    }
}

fn handle_chat(
    event: On<Received<ServerboundChat<'static>>>,
    sender_query: Query<(&GameProfile, Option<&ClientInfo>, &HostAnchor)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(pkt) = event.packet() else {
        return;
    };
    let Ok((profile, info, host_anchor)) = sender_query.get(event.entity) else {
        return;
    };
    let msg = pkt.message;
    if !msg.chars().all(is_allowed_chat_character) {
        info!(
            player = %profile.username,
            "dropping chat message with illegal characters"
        );
        return;
    }

    if info.is_some_and(|info| info.chat_mode == ChatMode::Hidden) {
        packet_writer.write(bus::to(
            host_anchor.0,
            system_chat(Text::translate("chat.disabled.options", vec![]).color(Color::RED)),
        ));
        return;
    }

    let text = Text::translate(
        "chat.type.text",
        vec![
            profile.username.clone().into_text(),
            msg.to_string().into_text(),
        ],
    );
    info!("<{}> {}", profile.username, msg);

    packet_writer.write(to_target(PacketTarget::AllPlayers, system_chat(text)));
}
