use crate::{BlockPos, SectionPos};
use core::fmt::Debug;
use glam::{DVec3, IVec2};

/// The X and Z position of a chunk column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ColumnPos {
    pub x: i32,
    pub z: i32,
}

impl Debug for ColumnPos {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (self.x, self.z).fmt(f)
    }
}

impl ColumnPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Saturates at `i32::MAX` for columns too far apart to square in an `i32`.
    pub const fn distance_squared(self, other: Self) -> i32 {
        let diff_x = other.x.saturating_sub(self.x);
        let diff_z = other.z.saturating_sub(self.z);

        diff_x
            .saturating_mul(diff_x)
            .saturating_add(diff_z.saturating_mul(diff_z))
    }

    /// The larger of the distances along the two axes, saturating at `i32::MAX`.
    pub fn chebyshev_distance(self, other: Self) -> i32 {
        let distance = self.x.abs_diff(other.x).max(self.z.abs_diff(other.z));
        i32::try_from(distance).unwrap_or(i32::MAX)
    }

    /// The columns within `radius` of this one on both axes, a square `2 * radius + 1` wide.
    /// A coordinate past the `i32` range stops at its end.
    pub fn square(self, radius: i32) -> impl Iterator<Item = Self> {
        (radius.saturating_neg()..=radius).flat_map(move |dz| {
            (radius.saturating_neg()..=radius)
                .map(move |dx| Self::new(self.x.saturating_add(dx), self.z.saturating_add(dz)))
        })
    }

    /// The same columns as [`ColumnPos::square`], nearest ring first: this column, then the
    /// eight around it, and so on out to `radius`.
    pub fn rings(self, radius: i32) -> impl Iterator<Item = Self> {
        (0..=radius).flat_map(move |ring| {
            let side = (-ring..=ring).flat_map(move |along| {
                [
                    Self::new(self.x.saturating_add(along), self.z.saturating_sub(ring)),
                    Self::new(self.x.saturating_add(along), self.z.saturating_add(ring)),
                ]
            });
            let ends = (1 - ring..ring).flat_map(move |along| {
                [
                    Self::new(self.x.saturating_sub(ring), self.z.saturating_add(along)),
                    Self::new(self.x.saturating_add(ring), self.z.saturating_add(along)),
                ]
            });
            // Ring 0 is the column itself, which `side` would yield twice.
            side.take(if ring == 0 { 1 } else { usize::MAX })
                .chain(ends)
        })
    }

    pub const fn min_block_x(self) -> i32 {
        self.x << SectionPos::BITS
    }

    pub const fn min_block_z(self) -> i32 {
        self.z << SectionPos::BITS
    }

    pub const fn middle_block_x(self) -> i32 {
        self.min_block_x() + 8
    }

    pub const fn middle_block_z(self) -> i32 {
        self.min_block_z() + 8
    }
}

impl From<SectionPos> for ColumnPos {
    fn from(pos: SectionPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

impl From<BlockPos> for ColumnPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            x: pos.x >> SectionPos::BITS,
            z: pos.z >> SectionPos::BITS,
        }
    }
}

impl From<IVec2> for ColumnPos {
    fn from(v: IVec2) -> Self {
        Self { x: v.x, z: v.y }
    }
}

impl From<DVec3> for ColumnPos {
    fn from(pos: DVec3) -> Self {
        let pos = pos.floor();
        Self {
            x: (pos.x as i32) >> SectionPos::BITS,
            z: (pos.z as i32) >> SectionPos::BITS,
        }
    }
}

impl From<(i32, i32)> for ColumnPos {
    fn from((x, z): (i32, i32)) -> Self {
        Self { x, z }
    }
}

impl From<ColumnPos> for (i32, i32) {
    fn from(pos: ColumnPos) -> Self {
        (pos.x, pos.z)
    }
}

impl From<[i32; 2]> for ColumnPos {
    fn from([x, z]: [i32; 2]) -> Self {
        Self { x, z }
    }
}

impl From<ColumnPos> for [i32; 2] {
    fn from(pos: ColumnPos) -> Self {
        [pos.x, pos.z]
    }
}

#[cfg(test)]
mod rings_tests {
    extern crate std;

    use super::ColumnPos;
    use std::vec::Vec;

    #[test]
    fn rings_cover_the_square_once_nearest_first() {
        let centre = ColumnPos::new(3, -2);
        for radius in 0..4 {
            let rings: Vec<ColumnPos> = centre.rings(radius).collect();
            let mut sorted = rings.clone();
            sorted.sort_unstable();
            let mut square: Vec<ColumnPos> = centre.square(radius).collect();
            square.sort_unstable();
            assert_eq!(sorted, square, "radius {radius}");
            assert!(
                rings
                    .windows(2)
                    .all(|pair| centre.chebyshev_distance(pair[0])
                        <= centre.chebyshev_distance(pair[1])),
                "radius {radius}"
            );
        }
    }
}
