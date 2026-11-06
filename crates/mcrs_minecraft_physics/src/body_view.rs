use mcrs_engine_muxer::EntityId;

use crate::Aabb;
use crate::BirthKey;
use crate::GridRow;

/// The other entities that a step reads. The driver picks the front grid with the end boxes of
/// the hard colliders, or a live view in the exact-parity mode.
pub trait BodyView {
    /// Appends the entities whose box intersects `query` and that the entity `mover` can collide
    /// with: not itself, not a spectator, a type that other entities collide with, and not on
    /// the same root vehicle.
    fn colliders(&self, query: Aabb, mover: BirthKey, out: &mut Vec<EntityCollider>);

    /// Appends the bodies whose box intersects the box of `of`, from the start of the tick. The
    /// list can hold `of` itself and bodies that only touch it; the push removes them.
    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>);
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

    fn neighbours(&self, _of: &GridRow, _out: &mut Vec<GridRow>) {}
}

#[cfg(feature = "testing")]
/// Bodies in a list, for tests: each query reads every body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListedBodies {
    pub colliders: Vec<EntityCollider>,
    pub rows: Vec<GridRow>,
}

#[cfg(feature = "testing")]
impl BodyView for ListedBodies {
    fn colliders(&self, query: Aabb, _mover: BirthKey, out: &mut Vec<EntityCollider>) {
        out.extend(
            self.colliders
                .iter()
                .filter(|found| found.bounds.intersects(&query)),
        );
    }

    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>) {
        let bounds = of.bounds();
        out.extend(
            self.rows
                .iter()
                .filter(|row| row.bounds().intersects(&bounds)),
        );
    }
}
