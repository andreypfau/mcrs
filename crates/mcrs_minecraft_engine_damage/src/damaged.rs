use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::HitReport;

#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Damaged {
    pub target: Entity,
    pub source: DamageSource,
    pub report: HitReport,
}
