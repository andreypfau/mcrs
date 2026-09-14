use crate::{BlockPos, RegionPos, SectionPos};
use bevy_math::{DVec3, IVec2};
use std::fmt::Debug;

/// The X and Z position of a chunk column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ColumnPos {
    pub x: i32,
    pub z: i32,
}

impl Debug for ColumnPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (self.x, self.z).fmt(f)
    }
}

impl ColumnPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub const fn distance_squared(self, other: Self) -> i32 {
        let diff_x = other.x - self.x;
        let diff_z = other.z - self.z;

        diff_x * diff_x + diff_z * diff_z
    }

    pub const fn region_local_x(self) -> i32 {
        self.x & RegionPos::MASK as i32
    }

    pub const fn region_local_z(self) -> i32 {
        self.z & RegionPos::MASK as i32
    }

    pub const fn manhattan_distance(&self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.z - other.z).abs()
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
        Self {
            x: (pos.x.floor() as i32) >> SectionPos::BITS,
            z: (pos.z.floor() as i32) >> SectionPos::BITS,
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
