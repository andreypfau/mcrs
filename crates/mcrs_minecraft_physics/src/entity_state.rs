use crate::BirthKey;
use crate::Body;
use crate::Living;
use crate::Rest;

/// The motion state of a server-simulated entity that is not a player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityState {
    pub key: BirthKey,
    pub body: Body,
    /// The state of a living entity; `None` for every other category.
    pub living: Option<Living>,
    pub no_gravity: bool,
    /// The number of ticks the entity has existed, before the increment of this tick.
    pub age: u32,
    /// The phase offset from the birth key, from 0 to 3.
    pub phase: u8,
    pub rest: Rest,
}

impl EntityState {
    #[must_use]
    pub fn new(key: BirthKey, body: Body, living: Option<Living>) -> Self {
        Self {
            key,
            body,
            living,
            no_gravity: false,
            age: 0,
            phase: 0,
            rest: Rest::Awake,
        }
    }
}
