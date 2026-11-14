use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{On, Query};
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundClientInformation;

/// Gives the player the view distance of each in-game client information of its client. The
/// view follows once per tick, so only the last of several in one tick counts.
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

/// The server clamps what a client asks for to its own limit, as vanilla does; ours is three
/// times vanilla's 32.
const MAX_VIEW_DISTANCE: u8 = 96;

/// The view distance `d` of a player whose client asks for `asked` columns.
pub(crate) fn clamp_view_distance(asked: u8) -> u8 {
    asked.clamp(2, MAX_VIEW_DISTANCE)
}
