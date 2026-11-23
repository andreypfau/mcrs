use bevy_ecs::change_detection::Tick;
use bevy_ecs::entity::Entity;
use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackKnockback {
    pub attacker: Entity,
    pub target: Entity,
    pub strength: f32,
    pub yaw: f32,
    pub position: DVec3,
    pub(crate) sprint_changed: Tick,
}
