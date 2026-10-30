use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, With};
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::network_id;
use mcrs_minecraft_engine::session::{GameProfile, HostAnchor};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_level::entity::EntityNetworkAddEvent;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::{ByteAngle, LpVec3, VarInt};

pub(crate) fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(&GameProfile, &Transform, &HostAnchor), With<Player>>,
    viewer: Query<&HostAnchor, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((profile, transform, &HostAnchor(anchor))) = added_player.get(event.entity) else {
        return;
    };
    let Ok(&HostAnchor(host_anchor)) = viewer.get(event.player) else {
        return;
    };

    packet_writer.write(to(
        host_anchor,
        PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id: VarInt(network_id::player_network_id(anchor)),
            uuid: profile.id,
            kind: EntityType::Player,
            pos: transform.translation,
            movement: LpVec3(DVec3::ZERO),
            yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            pitch: ByteAngle::from_degrees(transform.rotation.pitch()),
            head_yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            data: VarInt(0),
        }),
    ));
}
