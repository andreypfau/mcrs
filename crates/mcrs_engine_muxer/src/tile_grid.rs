use std::num::NonZeroU32;

use crate::tile_key::TileKey;

/// The size of a tile on each axis, in the units of the game. A game that wants one tile for the
/// full height of its world sets `y` to that height and passes `y` from the bottom of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileGrid {
    pub x: NonZeroU32,
    pub y: NonZeroU32,
    pub z: NonZeroU32,
}

impl TileGrid {
    /// The tile that holds the point `x`, `y`, `z`. Tiles start at the origin of the grid.
    #[must_use]
    pub fn tile_at(self, x: i32, y: i32, z: i32) -> TileKey {
        TileKey::new(
            floor_div(x, self.x),
            floor_div(y, self.y),
            floor_div(z, self.z),
        )
    }
}

fn floor_div(value: i32, size: NonZeroU32) -> i32 {
    let quotient = i64::from(value).div_euclid(i64::from(size.get()));
    // The quotient is between `value` and zero, so it always fits.
    i32::try_from(quotient).unwrap_or_default()
}
