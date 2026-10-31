use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{MessageWriter, On, Query, Res, With};
use mcrs_minecraft_command_engine::{OperatorBroadcast, system_chat};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketTarget, to_target};
use mcrs_minecraft_engine::ops::OpList;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_engine_world::session::Session;
use smallvec::SmallVec;

/// Delivers an admin message from any dimension to every online operator but its source. The
/// host holds each session's profile and the operator list, so the set is read here when a
/// message arrives.
pub fn broadcast_to_operators(
    broadcast: On<OperatorBroadcast>,
    sessions: Query<(Entity, &GameProfile), With<Session>>,
    ops: Option<Res<OpList>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(ops) = ops else {
        return;
    };
    let operators: SmallVec<[Entity; 8]> = sessions
        .iter()
        .filter(|&(anchor, profile)| anchor != broadcast.source && ops.get(&profile.id).is_some())
        .map(|(anchor, _)| anchor)
        .collect();
    if operators.is_empty() {
        return;
    }
    let Ok(message) = mcrs_minecraft_protocol::text::named(&broadcast.message) else {
        return;
    };
    writer.write(to_target(
        PacketTarget::PlayerSet(operators),
        system_chat(message),
    ));
}
