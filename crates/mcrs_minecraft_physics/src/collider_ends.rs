use std::collections::BTreeMap;

use mcrs_minecraft_entity_key::TileKey;

use crate::EntityCollider;

/// The end boxes of the hard colliders that the collider groups stepped, per tile. A tile is
/// ready when every group whose zones touch it has finished; a main-pass tile waits only for
/// those groups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColliderEnds {
    tiles: BTreeMap<TileKey, TileEnds>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct TileEnds {
    colliders: Vec<EntityCollider>,
    pending: u32,
}

impl ColliderEnds {
    /// Forgets the boxes of the last tick.
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// One more group must finish before `tile` is ready.
    pub fn expect(&mut self, tile: TileKey) {
        let ends = self.tiles.entry(tile).or_default();
        ends.pending = ends.pending.saturating_add(1);
    }

    /// A group finished in `tile` with these end boxes.
    pub fn finish(&mut self, tile: TileKey, colliders: impl IntoIterator<Item = EntityCollider>) {
        let ends = self.tiles.entry(tile).or_default();
        ends.colliders.extend(colliders);
        ends.pending = ends.pending.saturating_sub(1);
    }

    /// The end boxes in `tile`; `None` while a group that touches it has not finished.
    #[must_use]
    pub fn ready(&self, tile: TileKey) -> Option<&[EntityCollider]> {
        match self.tiles.get(&tile) {
            None => Some(&[]),
            Some(ends) if ends.pending == 0 => Some(&ends.colliders),
            Some(_) => None,
        }
    }
}
