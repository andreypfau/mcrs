use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Changed, Query, Res};
use mcrs_minecraft_command_engine::CommandTreePackets;
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::network_id;
use mcrs_minecraft_engine::session::HostAnchor;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;

/// The operator level that the client of a player was told, which the login and a dimension
/// change leave as it was told.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnnouncedOpLevel(pub PlayerOpLevel);

/// A player that spawns or changes dimension learns its level in the sequence that brings it
/// in; this tells it every later change, with the tree of its new level.
pub(crate) fn send_op_level(
    mut players: Query<
        (&PlayerOpLevel, &mut AnnouncedOpLevel, &HostAnchor),
        Changed<PlayerOpLevel>,
    >,
    tree: Option<Res<CommandTreePackets>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (op_level, mut announced, &HostAnchor(host)) in &mut players {
        if announced.0 == *op_level {
            continue;
        }
        announced.0 = *op_level;
        level_packets(host, *op_level, tree.as_deref(), |packet| {
            packet_writer.write(packet);
        });
    }
}

/// The level of a player, then the command tree it can see at that level.
pub(crate) fn level_packets(
    host: Entity,
    op_level: PlayerOpLevel,
    tree: Option<&CommandTreePackets>,
    mut push: impl FnMut(OutboundPlayerPacket),
) {
    push(
        to(
            host,
            PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent {
                entity_id: network_id::player_network_id(host),
                entity_status: op_level.entity_status(),
            }),
        )
        .critical(),
    );
    if let Some(packet) = tree.and_then(|tree| tree.to_player(host, op_level)) {
        push(packet);
    }
}
