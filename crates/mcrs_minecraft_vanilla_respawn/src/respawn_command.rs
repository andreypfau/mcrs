use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::observer::On;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::{ClientCommandAction, ServerboundClientCommand};

#[derive(Message, Clone, Copy, Debug)]
pub(crate) struct RespawnCommand {
    pub(crate) player: Entity,
}

pub(crate) fn on_client_command(
    received: On<Received<ServerboundClientCommand>>,
    mut commands: MessageWriter<RespawnCommand>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    if packet.action == ClientCommandAction::PerformRespawn {
        commands.write(RespawnCommand {
            player: received.entity,
        });
    }
}
