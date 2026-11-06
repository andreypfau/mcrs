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
    /// Columns past the `i32` range are left out.
    pub fn square(self, radius: i32) -> impl Iterator<Item = Self> {
        (radius.saturating_neg()..=radius)
            .filter_map(move |dz| self.z.checked_add(dz))
            .flat_map(move |z| {
                (radius.saturating_neg()..=radius)
                    .filter_map(move |dx| self.x.checked_add(dx))
                    .map(move |x| Self::new(x, z))
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
mod tests {
    use super::*;

    #[test]
    fn square_at_the_i32_edge_leaves_out_the_columns_past_it() {
        let expected = [
            ColumnPos::new(i32::MAX - 1, -1),
            ColumnPos::new(i32::MAX, -1),
            ColumnPos::new(i32::MAX - 1, 0),
            ColumnPos::new(i32::MAX, 0),
            ColumnPos::new(i32::MAX - 1, 1),
            ColumnPos::new(i32::MAX, 1),
        ];
        assert!(
            ColumnPos::new(i32::MAX, 0).square(1).eq(expected),
            "the square holds each in-range column once"
        );
    }
}
