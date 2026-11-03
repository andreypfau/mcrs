use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::network_id;
use crate::session::GameProfile;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, With};
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::EntityNetworkAddEvent;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3};

pub(crate) fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(&GameProfile, &Transform, &Owner), With<Player>>,
    viewer: Query<&Owner, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((profile, transform, &Owner(added))) = added_player.get(event.entity) else {
        return;
    };
    let Ok(&Owner(viewer)) = viewer.get(event.player) else {
        return;
    };

    packet_writer.write(to(
        viewer,
        PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id: VarInt(network_id::player_network_id(added)),
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
