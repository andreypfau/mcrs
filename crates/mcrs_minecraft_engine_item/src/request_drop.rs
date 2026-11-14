use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

use crate::RequestId;

/// A stack that a drop of an applied [`Request`](crate::Request) took out of its slot, written in
/// the order of the drops and before the [`RequestResult`](crate::RequestResult).
///
/// The stack has no placement and belongs to the requester, which places it in the world or
/// despawns it. A stack that no one places stays alive without placement.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestDrop {
    /// The holder whose `request` command gave the request.
    pub holder: Entity,
    pub id: RequestId,
    pub stack: Entity,
}
