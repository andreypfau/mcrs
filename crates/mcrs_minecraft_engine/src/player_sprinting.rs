use bevy_ecs::prelude::{Component, DetectChangesMut, On, Query, With};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::{PlayerCommandAction, ServerboundPlayerCommand};
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlayerSprinting(pub bool);

pub(crate) fn handle_player_command(
    event: On<Received<ServerboundPlayerCommand>>,
    mut players: Query<&mut PlayerSprinting, With<Owner>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let sprinting = match packet.action {
        PlayerCommandAction::StartSprinting => true,
        PlayerCommandAction::StopSprinting => false,
        _ => return,
    };
    if let Ok(mut state) = players.get_mut(event.entity) {
        state.set_if_neq(PlayerSprinting(sprinting));
    }
}
