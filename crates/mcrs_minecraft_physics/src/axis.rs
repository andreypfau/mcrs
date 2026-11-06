use glam::DVec3;

/// An axis of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    /// The two other axes, in the order `x`, `y`, `z`.
    #[must_use]
    pub const fn others(self) -> [Axis; 2] {
        match self {
            Axis::X => [Axis::Y, Axis::Z],
            Axis::Y => [Axis::X, Axis::Z],
            Axis::Z => [Axis::X, Axis::Y],
        }
    }

    #[must_use]
    pub const fn of(self, vector: DVec3) -> f64 {
        match self {
            Axis::X => vector.x,
            Axis::Y => vector.y,
            Axis::Z => vector.z,
        }
    }

    /// The element of `values` for this axis, with `values` in the order `x`, `y`, `z`.
    #[must_use]
    pub const fn pick<T>(self, values: &[T; 3]) -> &T {
        match self {
            Axis::X => &values[0],
            Axis::Y => &values[1],
            Axis::Z => &values[2],
        }
    }

    #[must_use]
    pub const fn with(self, mut vector: DVec3, value: f64) -> DVec3 {
        match self {
            Axis::X => vector.x = value,
            Axis::Y => vector.y = value,
            Axis::Z => vector.z = value,
        }
        vector
    }
}
