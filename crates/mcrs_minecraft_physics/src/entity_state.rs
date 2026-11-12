use mcrs_minecraft_entity_key::EntityKey;

use crate::Body;
use crate::HitRecord;
use crate::Living;
use crate::MovePath;
use crate::Rest;

/// The motion state of a server-simulated entity that is not a player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityState {
    pub key: EntityKey,
    pub body: Body,
    /// The state of a living entity; `None` for every other category.
    pub living: Option<Living>,
    /// The hits on a living entity that its AI reads; empty for every other category.
    pub hits: HitRecord,
    pub no_gravity: bool,
    /// The number of ticks the entity has existed, before the increment of this tick.
    pub age: u32,
    /// The phase offset of the UUID, from 0 to 3.
    pub phase: u8,
    pub rest: Rest,
    /// The path of the last tick that moved the entity. A resting item applies the blocks inside
    /// its box along this path. For a living entity the pass keeps the path of its last tick, of
    /// no length when that tick did not move it (PHY-CMP-027).
    pub last_path: MovePath,
}

impl EntityState {
    #[must_use]
    pub fn new(key: EntityKey, body: Body, living: Option<Living>) -> Self {
        Self {
            key,
            body,
            living,
            hits: HitRecord::default(),
            no_gravity: false,
            age: 0,
            phase: 0,
            rest: Rest::Awake,
            last_path: MovePath::at(body.position),
        }
    }
}
