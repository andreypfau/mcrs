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

    pub const fn distance_squared(self, other: Self) -> i32 {
        let diff_x = other.x - self.x;
        let diff_z = other.z - self.z;

        diff_x * diff_x + diff_z * diff_z
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
