use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{On, Query};
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundClientInformation;

pub(crate) fn on_client_view_distance(
    on: On<Received<ServerboundClientInformation<'static>>>,
    mut players: Query<&mut PlayerViewDistance>,
) {
    if let Some(packet) = on.packet()
        && let Ok(view) = players.get_mut(on.entity)
    {
        view.map_unchanged(|view| &mut view.distance)
            .set_if_neq(clamp_view_distance(packet.0.view_distance));
    }
}

const MAX_VIEW_DISTANCE: u8 = 96;

pub(crate) fn clamp_view_distance(asked: u8) -> u8 {
    asked.clamp(2, MAX_VIEW_DISTANCE)
}
