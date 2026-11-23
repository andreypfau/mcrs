use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use glam::DVec3;
use mcrs_minecraft_damage::DamageSource;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Died {
    pub target: Entity,
    pub position: DVec3,
    pub source: DamageSource,
}
