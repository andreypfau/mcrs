use std::collections::BTreeMap;

use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_replication::MobLooks;
use rustc_hash::FxHashMap;

/// The cold state of each mob row of the world, by its id.
#[derive(Resource, Debug, Default)]
pub struct Mobs(pub BTreeMap<EntityId, Mob>);

/// The id of each mob row of the world, by its UUID.
#[derive(Resource, Debug, Default)]
pub struct MobUuids(pub FxHashMap<u128, EntityId>);

/// What a mob keeps outside its row.
#[derive(Clone, Debug, PartialEq)]
pub struct Mob {
    pub kind: MobType,
    pub uuid: u128,
    pub looks: MobLooks,
    /// The tile of the row at the end of the last tick.
    pub tile: TileKey,
    pub custom_name: Option<String>,
    /// The scoreboard tags, which a selector tests.
    pub tags: Vec<String>,
    /// The last hit that the damage cooldown accepted (`mob-goals.md`, GOAL-PNC-001).
    pub last_hit: Option<MobHit>,
}

/// A hit on a mob: its tick and the network id of its damage type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobHit {
    pub tick: u64,
    pub damage_type: u32,
}
