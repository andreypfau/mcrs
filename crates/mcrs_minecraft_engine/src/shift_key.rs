use bevy_ecs::prelude::{Component, DetectChangesMut, Entity, On, Query, With};
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::{ServerboundInteract, ServerboundPlayerInput};
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ShiftKey(pub bool);

pub(crate) fn handle_player_input(
    event: On<Received<ServerboundPlayerInput>>,
    mut players: Query<&mut ShiftKey, With<Owner>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    set_shift(players.reborrow(), event.entity, packet.input.shift());
}

pub(crate) fn handle_interact(
    event: On<Received<ServerboundInteract>>,
    mut players: Query<&mut ShiftKey, With<Owner>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    set_shift(
        players.reborrow(),
        event.entity,
        packet.using_secondary_action,
    );
}

fn set_shift(mut players: Query<'_, '_, &mut ShiftKey, With<Owner>>, entity: Entity, shift: bool) {
    if let Ok(mut state) = players.get_mut(entity) {
        state.set_if_neq(ShiftKey(shift));
    }
}
