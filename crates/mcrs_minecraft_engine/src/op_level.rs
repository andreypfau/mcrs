use crate::ability::{OpLevelAnnounced, PlayerOpLevel};
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::network_id;
use crate::session::HostAnchor;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Query};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;

/// The operator level that the client of a player was told, which the login and a dimension
/// change leave as it was told.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnnouncedOpLevel(pub PlayerOpLevel);

/// A player that spawns or changes dimension learns its level in the sequence that brings it
/// in; this tells it every later change.
pub(crate) fn send_op_level(
    mut players: Query<
        (Entity, &PlayerOpLevel, &mut AnnouncedOpLevel, &HostAnchor),
        Changed<PlayerOpLevel>,
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut announced_levels: MessageWriter<OpLevelAnnounced>,
) {
    for (player, op_level, mut announced, &HostAnchor(host)) in &mut players {
        if announced.0 == *op_level {
            continue;
        }
        announced.0 = *op_level;
        packet_writer.write(op_level_packet(host, *op_level));
        announced_levels.write(OpLevelAnnounced {
            player,
            host,
            level: *op_level,
        });
    }
}

/// The `entity_event` that tells a client its operator level.
pub(crate) fn op_level_packet(host: Entity, op_level: PlayerOpLevel) -> OutboundPlayerPacket {
    to(
        host,
        PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent {
            entity_id: network_id::player_network_id(host),
            entity_status: op_level.entity_status(),
        }),
    )
    .critical()
}
