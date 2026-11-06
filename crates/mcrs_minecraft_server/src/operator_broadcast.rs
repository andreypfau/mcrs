use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{MessageWriter, On, Res};
use mcrs_minecraft_command_engine::{CommandTargets, OperatorBroadcast, system_chat};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketTarget, to_target};
use mcrs_minecraft_engine::coordinator::ops::OpList;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_text::RawText;
use smallvec::SmallVec;

/// Delivers the admin messages of the commands of each dimension world to the operators.
#[derive(Clone)]
pub struct OperatorBroadcastPlugin;

impl Plugin for OperatorBroadcastPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(broadcast_to_operators);
    }
}

/// Delivers an admin message to every online operator of any dimension but its source. The
/// world holds a copy of the operator list and the players of every dimension in the targets of
/// its commands, so the set is read there when a message arrives.
pub fn broadcast_to_operators(
    broadcast: On<OperatorBroadcast>,
    targets: Res<CommandTargets>,
    ops: Option<Res<OpList>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(ops) = ops else {
        return;
    };
    let operators: SmallVec<[PlayerSession; 8]> = targets
        .players()
        .iter()
        .filter(|player| player.session != broadcast.source && ops.get(&player.uuid).is_some())
        .map(|player| player.session)
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
