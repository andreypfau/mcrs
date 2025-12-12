use crate::{BiomePos, Decode, Encode, Position};
use bevy_math::DVec3;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::ChunkPos;
use std::fmt::Debug;

/// The X and Z position of a chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Encode, Decode)]
pub struct ChunkColumnPos {
    /// The X position of the chunk.
    pub x: i32,
    /// The Z position of the chunk.
    pub z: i32,
}

impl Debug for ChunkColumnPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (self.x, self.z).fmt(f)
    }
}

impl ChunkColumnPos {
    /// Constructs a new chunk position.
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

impl From<BlockPos> for ChunkColumnPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(16),
            z: pos.z.div_euclid(16),
        }
    }
}

impl From<ChunkPos> for ChunkColumnPos {
    fn from(pos: ChunkPos) -> Self {
        Self { x: pos.x, z: pos.z }
    }
}

impl From<BiomePos> for ChunkColumnPos {
    fn from(pos: BiomePos) -> Self {
        Self {
            x: pos.x.div_euclid(4),
            z: pos.z.div_euclid(4),
        }
    }
}

impl From<DVec3> for ChunkColumnPos {
    fn from(pos: DVec3) -> Self {
        Self {
            x: (pos.x / 16.0).floor() as i32,
            z: (pos.z / 16.0).floor() as i32,
        }
    }
}

impl From<Position> for ChunkColumnPos {
    #[inline]
    fn from(pos: Position) -> Self {
        Self::from(*pos)
    }
}

impl From<(i32, i32)> for ChunkColumnPos {
    fn from((x, z): (i32, i32)) -> Self {
        Self { x, z }
    }
}

impl From<ChunkColumnPos> for (i32, i32) {
    fn from(pos: ChunkColumnPos) -> Self {
        (pos.x, pos.z)
    }
}

impl From<[i32; 2]> for ChunkColumnPos {
    fn from([x, z]: [i32; 2]) -> Self {
        Self { x, z }
    }
}

impl From<ChunkColumnPos> for [i32; 2] {
    fn from(pos: ChunkColumnPos) -> Self {
        [pos.x, pos.z]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_pos_round_trip_conv() {
        let p = ChunkColumnPos::new(rand::random(), rand::random());

        assert_eq!(ChunkColumnPos::from(<(i32, i32)>::from(p)), p);
        assert_eq!(ChunkColumnPos::from(<[i32; 2]>::from(p)), p);
    }
}
