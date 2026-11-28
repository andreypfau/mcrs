use core::num::NonZeroU32;

use glam::DVec3;

pub const TILE_WIDTH: NonZeroU32 = match NonZeroU32::new(32) {
    Some(width) => width,
    None => NonZeroU32::MIN,
};

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

    #[inline]
    #[must_use]
    pub fn at(position: DVec3) -> Self {
        let width = f64::from(TILE_WIDTH.get());
        Self::new(floor(position.x / width), 0, floor(position.z / width))
    }
}

fn floor(value: f64) -> i32 {
    let truncated = value as i32;
    truncated.saturating_sub(i32::from(f64::from(truncated) > value))
}
