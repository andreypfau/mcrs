use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_engine_world::entity::physics::Transform;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct RespawnRequest {
    pub player: Entity,
    pub dimension: Entity,
    pub transform: Transform,
}
