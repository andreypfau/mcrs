use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::session::PlayerSession;

/// Where a player of a dimension stands and how far it sees, for the muxer that serves its
/// view. `None` when the player left the dimension.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub session: PlayerSession,
    pub view: Option<ChunkTrackingView>,
}
