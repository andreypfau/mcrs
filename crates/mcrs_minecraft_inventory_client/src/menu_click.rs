use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_container_click::Click;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuClick {
    pub viewer: Entity,
    pub click: Click,
}
