use crate::login::GameProfile;
use crate::world::entity::player::DisconnectReason;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{InGameConnectionState, ServerSideConnection};
use mcrs_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_protocol::packets::game::serverbound::ServerboundChat;
use mcrs_protocol::setting::ChatMode;
use mcrs_protocol::text::{Color, IntoText};
use mcrs_protocol::{Text, WritePacket};
use tracing::info;

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_chat);
    }
}

pub trait SendMessage {
    fn send_system_message<'a>(&mut self, msg: impl IntoText<'a>);

    fn send_action_message<'a>(&mut self, msg: impl IntoText<'a>);
}

impl<T: WritePacket> SendMessage for T {
    fn send_system_message<'a>(&mut self, msg: impl IntoText<'a>) {
        self.write_packet(&ClientboundSystemChatPacket {
            content: msg.into_text(),
            overlay: false,
        })
    }

    fn send_action_message<'a>(&mut self, msg: impl IntoText<'a>) {
        self.write_packet(&ClientboundSystemChatPacket {
            content: msg.into_text(),
            overlay: true,
        })
    }
}

fn handle_chat(
    event: On<ReceivedPacketEvent>,
    chat_mode_query: Query<(&GameProfile, &ChatMode)>,
    mut con_query: Query<(&mut ServerSideConnection), With<InGameConnectionState>>,
    mut commands: Commands,
) {
    let Some(pkt) = event.decode::<ServerboundChat>() else {
        return;
    };
    let Ok((profile, &chat_mode)) = chat_mode_query.get(event.entity) else {
        return;
    };
    let msg = pkt.message;
    if is_chat_message_illegal(&msg) {
        commands
            .entity(event.entity)
            .insert(DisconnectReason(Text::translate(
                "multiplayer.disconnect.illegal_characters",
                vec![],
            )));
        return;
    }

    if chat_mode == ChatMode::Hidden {
        let Ok(mut con) = con_query.get_mut(event.entity) else {
            return;
        };
        con.send_system_message(Text::translate("chat.disabled.options", vec![]).color(Color::RED));
    } else {
        let text = Text::translate(
            "chat.type.text",
            vec![
                profile.username.clone().into_text(),
                msg.to_string().into_text(),
            ],
        );
        info!("<{}> {}", profile.username, msg);

        con_query.iter_mut().for_each(|mut con| {
            con.send_system_message(&text);
        });
    }
}

#[inline]
fn is_chat_message_illegal(msg: &str) -> bool {
    msg.chars().any(|c| !is_allowed_chat_character(c))
}

#[inline]
fn is_allowed_chat_character(ch: char) -> bool {
    !matches!(ch, '\0'..='\x1f' | '\x7f' | '§')
}
