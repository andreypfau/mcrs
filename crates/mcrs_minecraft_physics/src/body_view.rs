use mcrs_engine_muxer::EntityId;

use crate::Aabb;
use crate::BirthKey;

/// The other entities that a step reads. The driver picks the front grid with the end boxes of
/// the hard colliders, or a live view in the exact-parity mode.
pub trait BodyView {
    /// Appends the entities whose box intersects `query` and that the entity `mover` can collide
    /// with: not itself, not a spectator, a type that other entities collide with, and not on
    /// the same root vehicle.
    fn colliders(&self, query: Aabb, mover: BirthKey, out: &mut Vec<EntityCollider>);
}

/// An entity that can stop the box of another entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityCollider {
    pub bounds: Aabb,
    pub id: EntityId,
}

/// A view with no other entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoBodies;

impl BodyView for NoBodies {
    fn colliders(&self, _query: Aabb, _mover: BirthKey, _out: &mut Vec<EntityCollider>) {}
}
