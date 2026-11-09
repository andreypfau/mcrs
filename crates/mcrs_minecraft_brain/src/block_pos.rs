use glam::DVec3;

/// The position of a block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// The block that holds `position`.
    #[must_use]
    pub fn containing(position: DVec3) -> Self {
        Self::new(
            libm::floor(position.x) as i32,
            libm::floor(position.y) as i32,
            libm::floor(position.z) as i32,
        )
    }

    #[must_use]
    pub const fn offset(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            x: self.x.wrapping_add(dx),
            y: self.y.wrapping_add(dy),
            z: self.z.wrapping_add(dz),
        }
    }

    #[must_use]
    pub const fn below(&self) -> Self {
        self.offset(0, -1, 0)
    }

    /// The centre of the bottom face.
    #[must_use]
    pub fn bottom_centre(&self) -> DVec3 {
        DVec3::new(
            f64::from(self.x) + 0.5,
            f64::from(self.y),
            f64::from(self.z) + 0.5,
        )
    }
}
