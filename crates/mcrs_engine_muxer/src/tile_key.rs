/// The position of a tile on the grid of tiles, in tiles. The tile is the unit of entity
/// interest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl TileKey {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}
