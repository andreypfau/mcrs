use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::Message;

use crate::Refusal;
use crate::RequestId;

#[derive(Message, EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestResult {
    #[event_target]
    pub holder: Entity,
    pub id: RequestId,
    pub outcome: Result<(), Refusal>,
}
