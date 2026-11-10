//! The birth clock of the world: the birth tick that every new entity UUID hashes. It rises by
//! one at the start of every server tick, nothing else changes it, and the save keeps it, so the
//! inputs of a new UUID never repeat in one world.

use bevy_ecs::resource::Resource;

/// The value of the birth clock in the current tick.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BirthClock(pub u64);
