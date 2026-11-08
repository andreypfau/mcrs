use crate::Point;

/// A tile of the world: 2 × 2 chunk columns of the full height. The stage keeps the brains of
/// the mobs of one tile together, so a tile is its unit of parallel work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub x: i32,
    pub z: i32,
}

impl TileId {
    /// The side of a tile in blocks.
    pub const SIDE: i32 = 32;

    #[must_use]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// The tile that holds the column of `point`.
    #[must_use]
    pub fn at(point: Point) -> Self {
        let block = point.block();
        Self {
            x: block.x.div_euclid(Self::SIDE),
            z: block.z.div_euclid(Self::SIDE),
        }
    }
}
