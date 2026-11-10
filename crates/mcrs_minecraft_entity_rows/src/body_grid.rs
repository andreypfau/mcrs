use std::mem;

use mcrs_minecraft_entity_key::TileKey;
use rustc_hash::FxHashMap;

use crate::TileBodies;

/// The front buffer of the body grid: the bodies of each tile at the start of the tick. The
/// writer of a tile keeps its back buffer, writes it in the pass, and swaps it in at the freeze.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyGrid {
    tiles: FxHashMap<TileKey, TileBodies>,
}

static NO_BODIES: TileBodies = TileBodies {
    rows: Vec::new(),
    starts: Vec::new(),
};

impl BodyGrid {
    /// The bodies of `tile` at the start of the tick.
    #[must_use]
    pub fn front(&self, tile: TileKey) -> &TileBodies {
        self.tiles.get(&tile).unwrap_or(&NO_BODIES)
    }

    /// Makes `back` the front of `tile`, and gives the old front in `back`.
    pub fn swap(&mut self, tile: TileKey, back: &mut TileBodies) {
        mem::swap(self.tiles.entry(tile).or_default(), back);
    }
}
