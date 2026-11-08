/// A point in the world, in blocks, in f64.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point {
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub fn distance_squared(&self, other: Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    #[must_use]
    pub fn horizontal_distance_squared(&self, other: Point) -> f64 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        dx * dx + dz * dz
    }

    /// The block position that holds the point.
    #[must_use]
    pub fn block(&self) -> BlockPos {
        BlockPos::new(
            libm::floor(self.x) as i32,
            libm::floor(self.y) as i32,
            libm::floor(self.z) as i32,
        )
    }

    #[must_use]
    pub fn with_y(&self, y: f64) -> Self {
        Self { y, ..*self }
    }
}

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
    pub fn bottom_centre(&self) -> Point {
        Point::new(
            f64::from(self.x) + 0.5,
            f64::from(self.y),
            f64::from(self.z) + 0.5,
        )
    }
}
