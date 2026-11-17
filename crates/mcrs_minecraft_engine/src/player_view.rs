use bevy_ecs::message::Message;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::session::PlayerSession;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub session: PlayerSession,
    pub view: Option<ChunkTrackingView>,
}

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ViewerPosition {
    pub session: PlayerSession,
    pub position: DVec3,
}
