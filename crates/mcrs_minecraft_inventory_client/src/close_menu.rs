use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CloseMenu {
    pub viewer: Entity,
}
