use crate::ability::{OpLevelAnnounced, PlayerOpLevel};
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::coordinator::ops::{DefaultOpLevel, OpList};
use crate::network_id;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, DetectChangesMut, Populated, Query, Res, With};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_protocol_game::clientbound::ClientboundEntityEvent;

#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct AnnouncedOpLevel(pub PlayerOpLevel);

pub(crate) fn refresh_op_levels(
    ops: Res<OpList>,
    default_level: Res<DefaultOpLevel>,
    mut players: Query<(&EntityUuid, &mut PlayerOpLevel), With<Owner>>,
) {
    for (uuid, mut op_level) in &mut players {
        op_level.set_if_neq(ops.level_of(&uuid.0, *default_level));
    }
}

pub(crate) fn send_op_level(
    mut players: Populated<
        (Entity, &PlayerOpLevel, &mut AnnouncedOpLevel, &Owner),
        Changed<PlayerOpLevel>,
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    mut announced_levels: MessageWriter<OpLevelAnnounced>,
) {
    for (player, op_level, announced, &Owner(session)) in &mut players {
        if !announced
            .map_unchanged(|announced| &mut announced.0)
            .set_if_neq(*op_level)
        {
            continue;
        }
        packet_writer.write(op_level_packet(session, *op_level));
        announced_levels.write(OpLevelAnnounced {
            player,
            session,
            level: *op_level,
        });
    }
}

pub(crate) fn op_level_packet(
    session: PlayerSession,
    op_level: PlayerOpLevel,
) -> OutboundPlayerPacket {
    to(
        session,
        PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent {
            entity_id: network_id::player_network_id(session),
            entity_status: op_level.entity_status(),
        }),
    )
}
