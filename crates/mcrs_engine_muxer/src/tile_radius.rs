/// The distance in tiles on each axis inside which a viewer subscribes to a tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TileRadius {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

impl TileRadius {
    #[must_use]
    pub const fn new(x: u16, y: u16, z: u16) -> Self {
        Self { x, y, z }
    }
}
