use crate::login::GameProfile;
use crate::world::bus::{
    ArrivalCause, MovePayload, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use crate::world::entity::player::HostAnchor;
use bevy_app::{App, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::InTransit;
use mcrs_engine::session::{Owner, PlayerSession};
use mcrs_engine::world::in_flight::alloc_move_id;
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
    mut sender_query: Query<(&HostAnchor, &mut Transform, &GameProfile, &Owner)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    move_sender: Res<mcrs_engine::world::channels::FromDimSender<crate::world::channel_types::FromDim>>,
    mut commands: Commands,
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
            let Ok((host_anchor, mut transform, _, _)) = sender_query.get_mut(event.entity) else {
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
            session: PlayerSession(0),
            epoch: 0,
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
            session: PlayerSession(0),
            epoch: 0,
            });
            info!("teleported {:?} to {:?}", event.entity, pos);
        }
        Some("dim") => {
            let Some(raw) = parts.next() else {
                return;
            };
            let dim_name = match raw {
                "nether" | "the_nether" => "minecraft:the_nether".to_string(),
                "overworld" | "over" => "minecraft:overworld".to_string(),
                "end" | "the_end" => "minecraft:the_end".to_string(),
                other if other.contains(':') => other.to_string(),
                other => format!("minecraft:{other}"),
            };
            let Ok((_host_anchor, _transform, profile, owner)) = sender_query.get(event.entity) else {
                return;
            };
            let session = owner.0;
            // Source-allocated id: stamp the in-transit entity with the same id
            // the host echoes back on confirm/rollback so the source can match it.
            let move_id = alloc_move_id();
            let payload = MovePayload::Player {
                uuid: profile.id,
                username: profile.username.clone(),
            };
            info!("dim move {:?} -> {}", event.entity, dim_name);
            // Hide-on-move-out: keep the source entity live but excluded from every
            // source-dim system until the target confirms (despawn) or the move is
            // rolled back (un-hide). The entity is never despawned here.
            commands.entity(event.entity).insert(InTransit { move_id });
            let _ = move_sender.0.try_send(crate::world::channel_types::FromDim::MoveEntity {
                move_id,
                target: dim_name,
                cause: ArrivalCause::CommandTeleport {
                    pos: DVec3::new(0.0, 100.0, 0.0),
                },
                payload,
                player: Some(session),
            });
        }
        _ => {}
    }
}

fn handle_chat(
    event: On<ReceivedPacketEvent>,
    sender_query: Query<(&GameProfile, Option<&ChatMode>, &HostAnchor)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(pkt) = event.decode::<ServerboundChat>() else {
        return;
    };
    let Ok((profile, chat_mode, host_anchor)) = sender_query.get(event.entity) else {
        return;
    };
    let msg = pkt.message;
    if is_chat_message_illegal(&msg) {
        info!(
            player = %profile.username,
            "dropping chat message with illegal characters"
        );
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
        session: PlayerSession(0),
        epoch: 0,
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
    session: PlayerSession(0),
    epoch: 0,
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
