use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_damage::{DamageSource, Hit, HitOutcome};
use mcrs_minecraft_entity_key::EntityId;

/// The subject of a hit: a player entity or a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DamageTarget {
    Player(Entity),
    Row(EntityId),
}

/// A request to damage a subject.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DamageRequest {
    pub target: DamageTarget,
    pub hit: Hit,
}

impl DamageRequest {
    #[must_use]
    pub const fn player(target: Entity, hit: Hit) -> Self {
        Self {
            target: DamageTarget::Player(target),
            hit,
        }
    }

    #[must_use]
    pub const fn row(target: EntityId, hit: Hit) -> Self {
        Self {
            target: DamageTarget::Row(target),
            hit,
        }
    }
}

/// Emitted when a subject takes damage.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DamageDealt {
    pub target: DamageTarget,
    pub hit: Hit,
    pub outcome: HitOutcome,
    pub damage: f32,
}

/// Emitted when a subject dies.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DeathEvent {
    pub target: DamageTarget,
    pub source: DamageSource,
}

/// A request from a player to respawn after dying.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RespawnRequest {
    pub player: Entity,
}

/// A request to push a player away from a hit.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct KnockbackRequest {
    pub target: Entity,
    pub direction: bevy_math::Vec3,
    pub strength: f32,
}
