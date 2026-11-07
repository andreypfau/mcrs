use std::collections::BTreeMap;
use std::mem;

use mcrs_engine_muxer::TileKey;

use crate::GridRow;

/// The body grid, with a front and a back buffer per tile. The front buffer is the state at the
/// start of the tick; the task of a tile writes the back buffer of its tile, and the freeze swaps
/// the two.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyGrid {
    tiles: BTreeMap<TileKey, GridTile>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct GridTile {
    front: Vec<GridRow>,
    back: Vec<GridRow>,
    written: bool,
}

impl BodyGrid {
    /// The rows of `tile` at the start of the tick.
    #[must_use]
    pub fn front(&self, tile: TileKey) -> &[GridRow] {
        self.tiles.get(&tile).map_or(&[], |grid| &grid.front)
    }

    /// The back buffer of `tile`, empty, for the rows of the next tick.
    pub fn back_mut(&mut self, tile: TileKey) -> &mut Vec<GridRow> {
        let grid = self.tiles.entry(tile).or_default();
        if !grid.written {
            grid.back.clear();
            grid.written = true;
        }
        &mut grid.back
    }

    /// Makes the back buffer of each written tile its front buffer. A tile that nobody wrote keeps
    /// its front buffer.
    pub fn swap(&mut self) {
        for grid in self.tiles.values_mut() {
            if mem::take(&mut grid.written) {
                mem::swap(&mut grid.front, &mut grid.back);
            }
        }
    }
}
