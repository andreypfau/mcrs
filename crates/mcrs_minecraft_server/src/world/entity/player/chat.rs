use crate::login::GameProfile;
use crate::world::bus::{
    ArrivalCause, MovePayload, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};
use crate::world::entity::player::HostAnchor;
use bevy_app::{App, Plugin};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, IVec3};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_level::entity::InTransit;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::session::{Owner, PlayerSession};
use mcrs_minecraft_level::world::in_flight::MoveIds;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::Text;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChat, ServerboundChatCommand,
};
use mcrs_minecraft_protocol::setting::ChatMode;

use crate::client_info::ClientInfo;
use mcrs_minecraft_protocol::text::{Color, IntoText};
use mcrs_minecraft_worldgen_generator::stages::FillContext;
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
    move_sender: Res<
        mcrs_minecraft_level::world::channels::FromDimSender<crate::world::channel_types::FromDim>,
    >,
    mut commands: Commands,
    fill: Option<Res<FillContext>>,
    mut move_ids: ResMut<MoveIds>,
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
                    content: format!("Teleported to {:.1}, {:.1}, {:.1}", pos.x, pos.y, pos.z)
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
            let Ok((_host_anchor, _transform, profile, owner)) = sender_query.get(event.entity)
            else {
                return;
            };
            let session = owner.0;
            // Source-allocated id: stamp the in-transit entity with the same id
            // the host echoes back on confirm/rollback so the source can match it.
            let move_id = move_ids.allocate();
            let payload = MovePayload::Player {
                uuid: profile.id,
                username: profile.username.clone(),
            };
            info!("dim move {:?} -> {}", event.entity, dim_name);
            // Hide-on-move-out: keep the source entity live but excluded from every
            // source-dim system until the target confirms (despawn) or the move is
            // rolled back (un-hide). The entity is never despawned here.
            commands.entity(event.entity).insert(InTransit { move_id });
            let _ = move_sender
                .0
                .try_send(crate::world::channel_types::FromDim::MoveEntity {
                    move_id,
                    target: dim_name,
                    cause: ArrivalCause::CommandTeleport {
                        pos: DVec3::new(0.0, 128.0, 0.0),
                    },
                    payload,
                    player: Some(session),
                });
        }
        Some("locate") => {
            let (Some("structure"), Some(raw)) = (parts.next(), parts.next()) else {
                return;
            };
            let Ok((host_anchor, transform, _, _)) = sender_query.get(event.entity) else {
                return;
            };
            let id = if raw.contains(':') {
                raw.to_string()
            } else {
                format!("minecraft:{raw}")
            };
            let origin = transform.translation.floor().as_ivec3();
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(host_anchor.0),
                priority: PacketPriority::Normal,
                data: PacketPayload::SystemChat {
                    content: locate_structure(fill.as_deref(), origin, &id),
                    overlay: false,
                },
                session: PlayerSession(0),
                epoch: 0,
            });
        }
        _ => {}
    }
}

fn locate_structure(fill: Option<&FillContext>, origin: IVec3, id: &str) -> Text {
    let not_found = || {
        Text::translate(
            "commands.locate.structure.not_found",
            vec![id.to_string().into_text()],
        )
        .color(Color::RED)
    };
    let Some(index) = fill.and_then(|fill| fill.structures.as_deref()) else {
        return not_found();
    };
    let Some(structure) = ResourceLocation::parse(id)
        .ok()
        .and_then(|location| index.tables().frozen.structure_ids.get(&location).copied())
    else {
        return Text::translate(
            "commands.locate.structure.invalid",
            vec![id.to_string().into_text()],
        )
        .color(Color::RED);
    };
    let Some((found, _)) = index.locate(origin, &[structure]) else {
        return not_found();
    };
    let dx = found.x.wrapping_sub(origin.x);
    let dz = found.z.wrapping_sub(origin.z);
    let distance = (dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz)) as f32)
        .sqrt()
        .floor() as i32;
    let coordinates = Text::translate(
        "chat.square_brackets",
        vec![Text::translate(
            "chat.coordinates",
            vec![
                found.x.to_string().into_text(),
                "~".into_text(),
                found.z.to_string().into_text(),
            ],
        )],
    )
    .color(Color::GREEN)
    .on_click_suggest_command(format!("/tp @s {} ~ {}", found.x, found.z))
    .on_hover_show_text(Text::translate("chat.coordinates.tooltip", vec![]));
    Text::translate(
        "commands.locate.structure.success",
        vec![
            id.to_string().into_text(),
            coordinates,
            distance.to_string().into_text(),
        ],
    )
}

fn handle_chat(
    event: On<ReceivedPacketEvent>,
    sender_query: Query<(&GameProfile, Option<&ClientInfo>, &HostAnchor)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(pkt) = event.decode::<ServerboundChat>() else {
        return;
    };
    let Ok((profile, info, host_anchor)) = sender_query.get(event.entity) else {
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

    if info.is_some_and(|info| info.chat_mode == ChatMode::Hidden) {
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
