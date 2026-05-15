use crate::geometry::{BlockPos, ChunkPos};
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

    pub const fn manhattan_distance(&self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.z - other.z).abs()
    }
}

impl From<ChunkPos> for ColumnPos {
    fn from(pos: ChunkPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

impl From<BlockPos> for ColumnPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(16),
            z: pos.z.div_euclid(16),
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
            x: (pos.x / 16.0).floor() as i32,
            z: (pos.z / 16.0).floor() as i32,
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
