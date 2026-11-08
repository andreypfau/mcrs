use glam::DVec3;

use crate::Axis;

/// An axis-aligned box in f64 world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    #[must_use]
    pub const fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    /// The box of a body at `position`, the centre of its bottom face. The half width is an f32
    /// value, widened.
    #[must_use]
    pub fn of_body(position: DVec3, width: f32, height: f32) -> Self {
        let half_width = f64::from(width / 2.0);
        Self {
            min: DVec3::new(position.x - half_width, position.y, position.z - half_width),
            max: DVec3::new(
                position.x + half_width,
                position.y + f64::from(height),
                position.z + half_width,
            ),
        }
    }

    /// Moves one face for each component: a negative component the minimum face, a positive one
    /// the maximum face.
    #[must_use]
    pub fn expand_towards(self, by: DVec3) -> Self {
        let mut grown = self;
        for axis in Axis::ALL {
            let amount = axis.of(by);
            if amount < 0.0 {
                grown.min = axis.with(grown.min, axis.of(grown.min) + amount);
            } else if amount > 0.0 {
                grown.max = axis.with(grown.max, axis.of(grown.max) + amount);
            }
        }
        grown
    }

    /// Grows each face outwards by `amount` on its axis.
    #[must_use]
    pub fn inflate(self, amount: DVec3) -> Self {
        Self {
            min: self.min - amount,
            max: self.max + amount,
        }
    }

    #[must_use]
    pub fn deflate(self, amount: f64) -> Self {
        self.inflate(DVec3::splat(-amount))
    }

    #[must_use]
    pub fn translate(self, by: DVec3) -> Self {
        Self {
            min: self.min + by,
            max: self.max + by,
        }
    }

    #[must_use]
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }

    #[must_use]
    pub fn min_of(&self, axis: Axis) -> f64 {
        axis.of(self.min)
    }

    #[must_use]
    pub fn max_of(&self, axis: Axis) -> f64 {
        axis.of(self.max)
    }

    /// True when the two boxes overlap with a volume that is more than zero.
    #[must_use]
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}
