use crate::{BlockPos, ChunkPos};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl CellPos {
    pub const BITS: usize = 2;
    pub const SIZE: usize = 1 << Self::BITS;
    pub const AREA: usize = Self::SIZE * Self::SIZE;
    pub const VOLUME: usize = Self::AREA * Self::SIZE;
    pub const HALF_SIZE: usize = Self::SIZE >> 1;
    pub const HALF_VOLUME: usize = Self::VOLUME >> 1;
    pub const MASK: usize = Self::SIZE - 1;
    pub const CHUNK_TO_CELL_BITS: usize = ChunkPos::BITS - Self::BITS;

    pub const INVALID: CellPos = CellPos {
        x: i32::MIN,
        y: i32::MIN,
        z: i32::MIN,
    };

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

impl From<BlockPos> for CellPos {
    fn from(value: BlockPos) -> Self {
        Self {
            x: value.x >> CellPos::BITS,
            y: value.y >> CellPos::BITS,
            z: value.z >> CellPos::BITS,
        }
    }
}

impl From<CellPos> for BlockPos {
    fn from(value: CellPos) -> Self {
        Self {
            x: value.x << CellPos::BITS,
            y: value.y << CellPos::BITS,
            z: value.z << CellPos::BITS,
        }
    }
}

impl From<ChunkPos> for CellPos {
    fn from(value: ChunkPos) -> Self {
        Self {
            x: value.x << CellPos::CHUNK_TO_CELL_BITS,
            y: value.y << CellPos::CHUNK_TO_CELL_BITS,
            z: value.z << CellPos::CHUNK_TO_CELL_BITS,
        }
    }
}

impl From<CellPos> for ChunkPos {
    fn from(value: CellPos) -> Self {
        Self {
            x: value.x >> CellPos::CHUNK_TO_CELL_BITS,
            y: value.y >> CellPos::CHUNK_TO_CELL_BITS,
            z: value.z >> CellPos::CHUNK_TO_CELL_BITS,
        }
    }
}
