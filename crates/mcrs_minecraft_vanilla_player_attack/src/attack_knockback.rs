use bevy_ecs::change_detection::Tick;
use bevy_ecs::entity::Entity;
use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AttackKnockback {
    pub(crate) attacker: Entity,
    pub(crate) attacker_uuid: EntityUuid,
    pub(crate) target: Entity,
    pub(crate) strength: f32,
    pub(crate) yaw: f32,
    pub(crate) position: DVec3,
    pub(crate) sprint_changed: Tick,
}
