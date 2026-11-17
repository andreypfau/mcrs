use std::collections::BTreeMap;

use mcrs_minecraft_entity_key::TileKey;

use crate::EntityCollider;

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
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    pub fn expect(&mut self, tile: TileKey) {
        let ends = self.tiles.entry(tile).or_default();
        ends.pending = ends.pending.saturating_add(1);
    }

    pub fn finish(&mut self, tile: TileKey, colliders: impl IntoIterator<Item = EntityCollider>) {
        let ends = self.tiles.entry(tile).or_default();
        ends.colliders.extend(colliders);
        ends.pending = ends.pending.saturating_sub(1);
    }

    #[must_use]
    pub fn ready(&self, tile: TileKey) -> Option<&[EntityCollider]> {
        match self.tiles.get(&tile) {
            None => Some(&[]),
            Some(ends) if ends.pending == 0 => Some(&ends.colliders),
            Some(_) => None,
        }
    }
}
