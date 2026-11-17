use bevy::prelude::*;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::{AttributeInstances, InstanceChange};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundLogin, ClientboundRespawn, ClientboundUpdateAttributes,
};

use crate::joined_game::JoinedGame;
use crate::local_player::Sprint;
use crate::player::Player;

pub(crate) fn on_login(
    _event: On<Received<ClientboundLogin>>,
    mut player: Query<(Entity, Option<&mut Sprint>), With<Player>>,
    mut commands: Commands,
) {
    let Ok((player, sprint)) = player.single_mut() else {
        return;
    };
    if let Some(mut sprint) = sprint {
        sprint.set_if_neq(Sprint::default());
    }
    if let Some(bases) = mcrs_minecraft_entity::attribute_bases(EntityType::Player) {
        commands
            .entity(player)
            .insert(AttributeInstances::new(bases));
    }
}

pub(crate) fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    mut player: Query<(Option<&mut AttributeInstances>, Option<&mut Sprint>), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((instances, sprint)) = player.single_mut() else {
        return;
    };
    if let Some(mut sprint) = sprint {
        sprint.set_if_neq(Sprint::default());
    }
    if packet.data_to_keep & ClientboundRespawn::KEEP_ATTRIBUTE_MODIFIERS == 0
        && let Some(mut instances) = instances
        && instances.bypass_change_detection().clear_modifiers() == InstanceChange::Changed
    {
        instances.set_changed();
    }
}

pub(crate) fn on_update_attributes(
    event: On<Received<ClientboundUpdateAttributes<'static>>>,
    connections: Query<&JoinedGame>,
    mut player: Query<&mut AttributeInstances, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == packet.entity_id.0);
    if !own {
        return;
    }
    let Ok(mut instances) = player.single_mut() else {
        return;
    };
    let mut change = InstanceChange::Unchanged;
    for entry in &packet.attributes {
        match instances.bypass_change_detection().receive(entry) {
            Ok(InstanceChange::Changed) => change = InstanceChange::Changed,
            Ok(InstanceChange::Unchanged) => {}
            Err(error) => {
                warn!(%error, "the server sent an attribute entry that the client skips");
            }
        }
    }
    if change == InstanceChange::Changed {
        instances.set_changed();
    }
}
