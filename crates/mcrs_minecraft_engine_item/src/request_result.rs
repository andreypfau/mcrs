use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::Message;

use crate::Rejected;
use crate::RequestId;

/// The result of a [`Request`](crate::Request): applied in full, or rejected with no change. It is
/// written as a message and triggered as an event on `holder`.
#[derive(Message, EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestResult {
    /// The holder whose `request` command gave the request.
    #[event_target]
    pub holder: Entity,
    pub id: RequestId,
    pub outcome: Result<(), Rejected>,
}
