use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::ops::{Add, Sub};
use bitfield_struct::bitfield;
use derive_more::From;
use thiserror::Error;
use valence_math::{DVec3, IVec3};
use crate::{BiomePos, BlockPos, ChunkColumnPos, Decode, Encode, Position};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    bevy_ecs::component::Component,
    bevy_reflect::Reflect,
)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Default for ChunkPos {
    fn default() -> Self {
        ChunkPos::INVALID
    }
}

impl ChunkPos {
    pub const BITS: usize = 4;
    pub const SIZE: usize = 1 << Self::BITS;
    pub const AREA: usize = Self::SIZE * Self::SIZE;
    pub const VOLUME: usize = Self::AREA * Self::SIZE;
    pub const HALF_SIZE: usize = Self::SIZE >> 1;
    pub const HALF_VOLUME: usize = Self::VOLUME >> 1;
    pub const MASK: usize = Self::SIZE - 1;

    pub const INVALID: ChunkPos = ChunkPos {
        x: i32::MIN,
        y: i32::MIN,
        z: i32::MIN,
    };

    const PACKED_X_LENGTH: usize = 22;
    const PACKED_Z_LENGTH: usize = 22;
    const PACKED_Y_LENGTH: usize = 20;
    const PACKED_X_MASK: u64 = (1 << Self::PACKED_X_LENGTH) - 1;
    const PACKED_Y_MASK: u64 = (1 << Self::PACKED_Y_LENGTH) - 1;
    const PACKED_Z_MASK: u64 = (1 << Self::PACKED_Z_LENGTH) - 1;

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn packed(self) -> Result<PackedChunkSectionPos, Error> {
        match (self.x, self.y, self.z) {
            (-2097152..=2097151, -524288..=524287, -2097152..=2097151) => {
                Ok(PackedChunkSectionPos::new()
                    .with_x(self.x)
                    .with_y(self.y)
                    .with_z(self.z))
            }
            _ => Err(Error(self)),
        }
    }

    pub const fn manhattan_distance(&self, other: ChunkPos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }
}

impl Add for ChunkPos {
    type Output = ChunkPos;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Add<IVec3> for ChunkPos {
    type Output = ChunkPos;

    #[inline]
    fn add(self, rhs: IVec3) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for ChunkPos {
    type Output = ChunkPos;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl fmt::Display for ChunkPos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&(self.x, self.y, self.z), f)
    }
}

impl Hash for ChunkPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let packed = (self.x as u64 & Self::PACKED_X_MASK) << 42
            | (self.z as u64 & Self::PACKED_Z_MASK) << 20
            | (self.y as u64 & Self::PACKED_Y_MASK);
        packed.hash(state);
    }
}

impl Encode for ChunkPos {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.packed()?.encode(w)
    }
}

impl Decode<'_> for ChunkPos {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        PackedChunkSectionPos::decode(r).map(Into::into)
    }
}

impl From<BlockPos> for ChunkPos {
    fn from(pos: BlockPos) -> Self {
        Self {
            x: pos.x.div_euclid(16),
            y: pos.y.div_euclid(16),
            z: pos.z.div_euclid(16),
        }
    }
}

impl From<BiomePos> for ChunkPos {
    fn from(pos: BiomePos) -> Self {
        Self {
            x: pos.x.div_euclid(4),
            y: pos.y.div_euclid(4),
            z: pos.z.div_euclid(4),
        }
    }
}

impl From<DVec3> for ChunkPos {
    fn from(pos: DVec3) -> Self {
        Self {
            x: (pos.x / 16.0).floor() as i32,
            y: (pos.y / 16.0).floor() as i32,
            z: (pos.z / 16.0).floor() as i32,
        }
    }
}

impl From<Position> for ChunkPos {
    #[inline]
    fn from(pos: Position) -> Self {
        Self::from(*pos)
    }
}

#[bitfield(u64)]
#[derive(PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct PackedChunkSectionPos {
    #[bits(20)]
    pub y: i32,
    #[bits(22)]
    pub z: i32,
    #[bits(22)]
    pub x: i32,
}

impl From<PackedChunkSectionPos> for ChunkPos {
    fn from(pos: PackedChunkSectionPos) -> Self {
        Self {
            x: pos.x(),
            y: pos.y(),
            z: pos.z(),
        }
    }
}

impl TryFrom<ChunkPos> for PackedChunkSectionPos {
    type Error = Error;

    fn try_from(pos: ChunkPos) -> Result<Self, Self::Error> {
        pos.packed()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Error, From)]
#[error("chunk section position of {0} is out of range")]
pub struct Error(pub ChunkPos);
