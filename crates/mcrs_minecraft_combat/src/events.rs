use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_damage::{DamageSource, Hit, HitOutcome};

/// A request to damage an entity.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DamageRequest {
    pub target: Entity,
    pub hit: Hit,
}

impl DamageRequest {
    #[must_use]
    pub const fn new(target: Entity, hit: Hit) -> Self {
        Self { target, hit }
    }
}

/// Emitted when an entity takes damage.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DamageDealt {
    pub target: Entity,
    pub hit: Hit,
    pub outcome: HitOutcome,
    pub damage: f32,
}

/// A request to push an entity away from a hit, before its own physics step.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct KnockbackRequest {
    pub target: Entity,
    pub direction: bevy_math::Vec3,
    pub strength: f32,
}

/// Emitted when an entity dies.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DeathEvent {
    pub entity: Entity,
    pub damage_type: u32,
    pub source: DamageSource,
}

/// A request from a player to respawn after dying.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct RespawnRequest {
    pub player: Entity,
}
