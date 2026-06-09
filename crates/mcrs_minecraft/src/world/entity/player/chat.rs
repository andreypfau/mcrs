use crate::login::GameProfile;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::{DisconnectReason, HostAnchor};
use bevy_app::{App, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::{ServerboundChat, ServerboundChatCommand};
use mcrs_protocol::setting::ChatMode;
use mcrs_protocol::text::{Color, IntoText};
use mcrs_protocol::Text;
use tracing::info;

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_chat);
        app.add_observer(handle_command);
    }
}

/// Primitive debug slash-command handler. The vanilla client sends
/// `ServerboundChatCommand` for any `/...` input (no command graph is
/// required for the client to transmit it). Commands run inside the
/// dimension sub-app, so any client-facing effect must route through the
/// bridge (`OutboundPlayerPacket`) rather than touching `ServerSideConnection`
/// directly, which is host-resident.
fn handle_command(
    event: On<ReceivedPacketEvent>,
    mut sender_query: Query<(&HostAnchor, &mut Transform)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(pkt) = event.decode::<ServerboundChatCommand>() else {
        return;
    };
    let command: &str = pkt.command.0;
    info!("command from {:?}: /{}", event.entity, command);
    let mut parts = command.split_whitespace();
    match parts.next() {
        Some("tp") => {
            let coords: Vec<f64> = parts.filter_map(|s| s.parse::<f64>().ok()).collect();
            if coords.len() != 3 {
                return;
            }
            let pos = DVec3::new(coords[0], coords[1], coords[2]);
            let Ok((host_anchor, mut transform)) = sender_query.get_mut(event.entity) else {
                return;
            };
            let host = host_anchor.0;
            transform.translation = pos;
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(host),
                priority: PacketPriority::Critical,
                data: PacketPayload::PlayerPosition {
                    teleport_id: 1,
                    position: pos,
                },
            });
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(host),
                priority: PacketPriority::Normal,
                data: PacketPayload::SystemChat {
                    content: format!(
                        "Teleported to {:.1}, {:.1}, {:.1}",
                        pos.x, pos.y, pos.z
                    )
                    .into_text(),
                    overlay: false,
                },
            });
            info!("teleported {:?} to {:?}", event.entity, pos);
        }
        _ => {}
    }
}

fn handle_chat(
    event: On<ReceivedPacketEvent>,
    sender_query: Query<(&GameProfile, Option<&ChatMode>, &HostAnchor)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    let Some(pkt) = event.decode::<ServerboundChat>() else {
        return;
    };
    let Ok((profile, chat_mode, host_anchor)) = sender_query.get(event.entity) else {
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

    // ChatMode is never inserted today, so absence means "shown". Only an
    // explicit Hidden suppresses broadcast and echoes the disabled notice
    // back to the sender's own host connection.
    if chat_mode.copied() == Some(ChatMode::Hidden) {
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host_anchor.0),
            priority: PacketPriority::Normal,
            data: PacketPayload::SystemChat {
                content: Text::translate("chat.disabled.options", vec![]).color(Color::RED),
                overlay: false,
            },
        });
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

    packet_writer.write(OutboundPlayerPacket {
        target: PacketTarget::AllPlayers,
        priority: PacketPriority::Normal,
        data: PacketPayload::SystemChat {
            content: text,
            overlay: false,
        },
    });
}

#[inline]
fn is_chat_message_illegal(msg: &str) -> bool {
    msg.chars().any(|c| !is_allowed_chat_character(c))
}

#[inline]
fn is_allowed_chat_character(ch: char) -> bool {
    !matches!(ch, '\0'..='\x1f' | '\x7f' | '§')
}
