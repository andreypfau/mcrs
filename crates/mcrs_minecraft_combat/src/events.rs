use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_damage::{Hit, HitOutcome};

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

/// Emitted when an entity dies.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct DeathEvent {
    pub entity: Entity,
}

/// A request from a player to respawn after dying.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct RespawnRequest {
    pub player: Entity,
}
