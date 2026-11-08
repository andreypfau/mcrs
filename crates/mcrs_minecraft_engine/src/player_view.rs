use bevy_ecs::message::Message;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::session::PlayerSession;

/// Where a player of a dimension stands and how far it sees, for the muxer that serves its
/// view. `None` when the player left the dimension.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub session: PlayerSession,
    pub view: Option<ChunkTrackingView>,
}

/// Where a player of a dimension stands, for the muxer that picks the clients near a block
/// event. Reported when the player moves.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ViewerPosition {
    pub session: PlayerSession,
    pub position: DVec3,
}
