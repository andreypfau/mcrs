use bevy_ecs::message::Message;
use glam::DVec3;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::BirthKey;

/// A request of a system to add a mob to the world (TRK-LIFE-002). The life cycle applies the
/// requests of a tick in the order of their birth keys.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct MobSpawnRequest {
    pub kind: MobType,
    pub position: DVec3,
    pub key: BirthKey,
    pub uuid: u128,
    pub setup: SpawnSetup,
}

/// Whether a new mob gets the first spawn setup of its type (TRK-SUM-002, ANIM-SPAWN-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpawnSetup {
    First,
    None,
}
