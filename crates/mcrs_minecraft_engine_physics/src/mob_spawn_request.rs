use bevy_ecs::message::Message;
use glam::DVec3;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::BirthKey;

/// A request of a system to add a mob to the world (TRK-LIFE-002). The life cycle applies the
/// requests of a tick in the order of their birth keys.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct MobSpawnRequest {
    pub kind: MobType,
    pub position: DVec3,
    pub key: BirthKey,
    pub uuid: u128,
    pub setup: SpawnSetup,
    pub extras: MobExtras,
}

/// The values of a spawn request that differ from the defaults of a new mob (TRK-SUM-005).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobExtras {
    pub no_ai: bool,
    pub custom_name: Option<String>,
    pub tags: Vec<String>,
}

/// Whether a new mob gets the first spawn setup of its type (TRK-SUM-002, ANIM-SPAWN-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpawnSetup {
    First,
    None,
}
