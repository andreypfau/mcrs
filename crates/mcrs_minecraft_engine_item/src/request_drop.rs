use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

use crate::RequestId;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestDrop {
    pub holder: Entity,
    pub id: RequestId,
    pub stack: Entity,
}
