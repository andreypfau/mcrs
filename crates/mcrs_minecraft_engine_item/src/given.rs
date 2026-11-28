use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Given {
    pub holder: Entity,
    pub stack: Option<Entity>,
    pub remainder: Option<Entity>,
}
