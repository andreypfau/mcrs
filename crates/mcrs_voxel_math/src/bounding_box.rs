use crate::{BlockPos, SectionPos};
use bevy_math::IVec3;

/// An inclusive box in block coordinates.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BoundingBox {
    pub min: BlockPos,
    pub max: BlockPos,
}

impl BoundingBox {
    pub fn point(pos: BlockPos) -> Self {
        Self { min: pos, max: pos }
    }

    pub fn from_corners(a: BlockPos, b: BlockPos) -> Self {
        Self {
            min: a.min(*b).into(),
            max: a.max(*b).into(),
        }
    }

    pub fn of_section(pos: SectionPos) -> Self {
        let min = pos.0 * SectionPos::SIZE as i32;
        Self {
            min: min.into(),
            max: (min + IVec3::splat(SectionPos::MASK as i32)).into(),
        }
    }

    pub fn moved(self, delta: IVec3) -> Self {
        Self {
            min: (*self.min + delta).into(),
            max: (*self.max + delta).into(),
        }
    }

    pub fn inflated(self, amount: i32) -> Self {
        Self {
            min: (*self.min - IVec3::splat(amount)).into(),
            max: (*self.max + IVec3::splat(amount)).into(),
        }
    }

    pub fn encapsulating(self, pos: BlockPos) -> Self {
        Self {
            min: self.min.min(*pos).into(),
            max: self.max.max(*pos).into(),
        }
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(*other.min).into(),
            max: self.max.max(*other.max).into(),
        }
    }

    pub fn clamp_y(self, min_y: i32, max_y: i32) -> Self {
        Self {
            min: BlockPos::new(self.min.x, self.min.y.max(min_y), self.min.z),
            max: BlockPos::new(self.max.x, self.max.y.min(max_y), self.max.z),
        }
    }

    /// Smallest section-aligned box containing this one.
    pub fn section_aligned(self) -> Self {
        Self::of_section(SectionPos::from(self.min))
            .union(Self::of_section(SectionPos::from(self.max)))
    }

    pub fn y_span(&self) -> i32 {
        self.max.y - self.min.y + 1
    }

    pub fn cells(self) -> u64 {
        let span = (*self.max - *self.min + IVec3::ONE).max(IVec3::ZERO);
        (span.x as u64)
            .saturating_mul(span.y as u64)
            .saturating_mul(span.z as u64)
    }

    pub fn intersects(self, other: Self) -> bool {
        (self.max.cmpge(*other.min) & self.min.cmple(*other.max)).all()
    }

    pub fn contains(self, other: Self) -> bool {
        (other.min.cmpge(*self.min) & other.max.cmple(*self.max)).all()
    }

    pub fn is_inside(self, pos: BlockPos) -> bool {
        self.distance_to(pos) == 0
    }

    /// L1 distance from a point to the nearest cell of the box.
    pub fn distance_to(self, pos: BlockPos) -> i32 {
        let below = (*self.min - *pos).max(IVec3::ZERO);
        let above = (*pos - *self.max).max(IVec3::ZERO);
        (below + above).element_sum()
    }

    pub fn corners(self) -> [BlockPos; 8] {
        let (lo, hi) = (self.min, self.max);
        [
            BlockPos::new(lo.x, lo.y, lo.z),
            BlockPos::new(hi.x, lo.y, lo.z),
            BlockPos::new(lo.x, hi.y, lo.z),
            BlockPos::new(hi.x, hi.y, lo.z),
            BlockPos::new(lo.x, lo.y, hi.z),
            BlockPos::new(hi.x, lo.y, hi.z),
            BlockPos::new(lo.x, hi.y, hi.z),
            BlockPos::new(hi.x, hi.y, hi.z),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(min: [i32; 3], max: [i32; 3]) -> BoundingBox {
        BoundingBox {
            min: min.into(),
            max: max.into(),
        }
    }

    #[test]
    fn a_section_spans_sixteen_blocks_on_negative_coordinates_too() {
        let bounds = BoundingBox::of_section(SectionPos::new(-1, 0, 2));
        assert_eq!(bounds, cube([-16, 0, 32], [-1, 15, 47]));
    }

    #[test]
    fn section_aligned_rounds_both_corners_outward() {
        let aligned = cube([-1, 3, 17], [0, 20, 17]).section_aligned();
        assert_eq!(aligned, cube([-16, 0, 16], [15, 31, 31]));
    }

    #[test]
    fn distance_is_zero_inside_and_l1_outside() {
        let bounds = cube([0, 0, 0], [4, 4, 4]);
        assert!(bounds.is_inside(BlockPos::new(4, 0, 2)));
        assert_eq!(bounds.distance_to(BlockPos::new(6, -1, 2)), 3);
    }

    #[test]
    fn touching_boxes_intersect_and_an_empty_box_has_no_cells() {
        let bounds = cube([0, 0, 0], [4, 2, 4]);
        assert!(bounds.intersects(cube([4, 2, 4], [9, 9, 9])));
        assert!(!bounds.intersects(cube([5, 0, 0], [9, 9, 9])));
        assert_eq!(cube([1, 0, 0], [0, 0, 0]).cells(), 0);
    }
}
