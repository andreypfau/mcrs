use bevy_ecs::prelude::{MessageWriter, On, Query, Res};
use mcrs_minecraft_command_engine::{OperatorBroadcast, system_chat};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketTarget, to_target};
use mcrs_minecraft_engine::coordinator::ops::OpList;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_world::session::{PlayerSession, Session};
use mcrs_minecraft_text::RawText;
use smallvec::SmallVec;

/// Delivers an admin message from any dimension to every online operator but its source. The
/// host holds each session's profile and the operator list, so the set is read here when a
/// message arrives.
pub fn broadcast_to_operators(
    broadcast: On<OperatorBroadcast>,
    sessions: Query<(&Session, &GameProfile)>,
    ops: Option<Res<OpList>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(ops) = ops else {
        return;
    };
    let operators: SmallVec<[PlayerSession; 8]> = sessions
        .iter()
        .filter(|&(session, profile)| {
            session.0 != broadcast.source && ops.get(&profile.id).is_some()
        })
        .map(|(session, _)| session.0)
        .collect();
    if operators.is_empty() {
        return;
    }
    let Ok(message) = RawText::from_text(&broadcast.message) else {
        return;
    };
    writer.write(to_target(
        PacketTarget::PlayerSet(operators),
        system_chat(message),
    ));
}
