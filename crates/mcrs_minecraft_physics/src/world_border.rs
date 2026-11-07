use glam::DVec3;

use crate::BorderRegion;

/// The world border of the current tick, as its edges on `x` and `z`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldBorder {
    pub min_x: f64,
    pub max_x: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl WorldBorder {
    /// The distance from the point to the nearest border line; negative outside the border.
    #[must_use]
    pub fn distance_inside(&self, at: DVec3) -> f64 {
        (at.x - self.min_x)
            .min(self.max_x - at.x)
            .min(at.z - self.min_z)
            .min(self.max_z - at.z)
    }

    /// True when the point is inside the border grown by `margin` on each side; the minimum lines
    /// are inside and the maximum lines outside.
    #[must_use]
    pub fn contains_with_margin(&self, at: DVec3, margin: f64) -> bool {
        at.x >= self.min_x - margin
            && at.x < self.max_x + margin
            && at.z >= self.min_z - margin
            && at.z < self.max_z + margin
    }

    /// The region of the border shape: the edges rounded out to whole blocks.
    #[must_use]
    pub fn region(&self) -> BorderRegion {
        BorderRegion {
            min_x: self.min_x.floor(),
            max_x: self.max_x.ceil(),
            min_z: self.min_z.floor(),
            max_z: self.max_z.ceil(),
        }
    }
}
