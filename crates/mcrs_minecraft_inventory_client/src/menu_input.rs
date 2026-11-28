use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

use crate::MenuAction;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct MenuInput {
    pub viewer: Entity,
    pub action: MenuAction,
}
