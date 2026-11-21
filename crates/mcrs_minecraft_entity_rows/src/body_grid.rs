use std::mem;

use mcrs_minecraft_entity_key::TileKey;
use rustc_hash::FxHashMap;

use crate::TileBodies;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyGrid {
    tiles: FxHashMap<TileKey, TileBodies>,
}

static NO_BODIES: TileBodies = TileBodies {
    rows: Vec::new(),
    starts: Vec::new(),
};

impl BodyGrid {
    #[must_use]
    pub fn front(&self, tile: TileKey) -> &TileBodies {
        self.tiles.get(&tile).unwrap_or(&NO_BODIES)
    }

    pub fn tiles(&self) -> impl Iterator<Item = (TileKey, &TileBodies)> {
        self.tiles.iter().map(|(tile, bodies)| (*tile, bodies))
    }

    pub fn swap(&mut self, tile: TileKey, back: &mut TileBodies) {
        mem::swap(self.tiles.entry(tile).or_default(), back);
    }
}
