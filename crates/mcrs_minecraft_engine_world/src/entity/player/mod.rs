use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::entity::physics::Transform;

pub mod chunk_view;

#[derive(Component, Default)]
pub struct Player;

/// Asks to move `player`, an entity of the world that writes the request, to the dimension `to`
/// at `transform`. A system of one dimension cannot reach another world, so a portal, and any
/// other move that a dimension decides inside its tick, writes this request. At the end of the
/// tick the world lets the player go, in the order of writing, and the player arrives in `to` on
/// the next tick, with the packets of a dimension change.
#[derive(Message, Clone, Debug)]
pub struct ChangeDimensionRequest {
    pub player: Entity,
    pub to: ResourceKeyBuf<Dimension>,
    pub transform: Transform,
}
