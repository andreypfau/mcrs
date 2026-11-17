use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_pose::Dimensions;
use mcrs_minecraft_protocol::uuid::Uuid;

#[derive(Clone, Debug)]
pub struct PlayerEntry {
    pub session: PlayerSession,
    pub dimension: usize,
    pub entity: Option<Entity>,
    pub uuid: Uuid,
    pub name: String,
    pub transform: Transform,
    pub mode: GameMode,
    pub dimensions: Dimensions,
}
