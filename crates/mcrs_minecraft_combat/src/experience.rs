use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;

/// Message emitted when a player gains experience.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ExperienceGainedEvent {
    pub player: Entity,
    pub amount: i32,
    pub new_level: i32,
    pub new_progress: f32,
}
