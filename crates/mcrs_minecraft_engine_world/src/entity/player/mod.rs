use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKey;

use crate::entity::physics::Transform;

pub mod chunk_view;

#[derive(Component, Default)]
pub struct Player;

/// Asks the host to move `player`, an entity of the world that writes the request, to the
/// dimension `to` at `transform`. A system of one dimension cannot reach another world, so a
/// portal, and any other move that a dimension decides inside its tick, writes this request.
/// The host applies the requests between two ticks, in the order of the dimensions and then in
/// the order of writing. The player arrives with the packets of a dimension change.
#[derive(Message, Clone, Debug)]
pub struct ChangeDimensionRequest {
    pub player: Entity,
    pub to: ResourceKey<Dimension>,
    pub transform: Transform,
}
