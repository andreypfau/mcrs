use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::entity::physics::Transform;

pub mod chunk_view;

#[derive(Component, Default)]
pub struct Player;

#[derive(Message, Clone, Debug)]
pub struct ChangeDimensionRequest {
    pub player: Entity,
    pub to: ResourceKeyBuf<Dimension>,
    pub transform: Transform,
}
