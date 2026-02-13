use std::io::Write;

use crate::{Decode, Encode};
use anyhow::bail;
use bitfield_struct::bitfield;
use derive_more::From;
use mcrs_engine::world::block::BlockPos;
use thiserror::Error;

#[bitfield(u64)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct PackedBlockPos {
    #[bits(12)]
    pub y: i32,
    #[bits(26)]
    pub z: i32,
    #[bits(26)]
    pub x: i32,
}

impl Encode for BlockPos {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        match PackedBlockPos::try_from(*self) {
            Ok(p) => p.encode(w),
            Err(e) => bail!("{e}: {self}"),
        }
    }
}

impl Decode<'_> for BlockPos {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        PackedBlockPos::decode(r).map(Into::into)
    }
}

impl From<PackedBlockPos> for BlockPos {
    fn from(p: PackedBlockPos) -> Self {
        Self::new(p.x(), p.y(), p.z())
    }
}

impl TryFrom<BlockPos> for PackedBlockPos {
    type Error = Error;

    fn try_from(pos: BlockPos) -> Result<Self, Self::Error> {
        PackedBlockPos::try_from(&pos)
    }
}

impl TryFrom<&BlockPos> for PackedBlockPos {
    type Error = Error;

    fn try_from(pos: &BlockPos) -> Result<Self, Self::Error> {
        match (pos.x, pos.y, pos.z) {
            (-0x2000000..=0x1ffffff, -0x800..=0x7ff, -0x2000000..=0x1ffffff) => {
                Ok(PackedBlockPos::new()
                    .with_x(pos.x)
                    .with_y(pos.y)
                    .with_z(pos.z))
            }
            _ => Err(Error(*pos)),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Error, From)]
#[error("block position of {0} is out of range")]
pub struct Error(pub BlockPos);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_position() {
        let xzs = [
            (-33554432, true),
            (-33554433, false),
            (33554431, true),
            (33554432, false),
            (0, true),
            (1, true),
            (-1, true),
        ];
        let ys = [
            (-2048, true),
            (-2049, false),
            (2047, true),
            (2048, false),
            (0, true),
            (1, true),
            (-1, true),
        ];

        for (x, x_valid) in xzs {
            for (y, y_valid) in ys {
                for (z, z_valid) in xzs {
                    let pos = BlockPos::new(x, y, z);
                    if x_valid && y_valid && z_valid {
                        let c = PackedBlockPos::try_from(pos).unwrap();
                        assert_eq!((c.x(), c.y(), c.z()), (pos.x, pos.y, pos.z));
                    } else {
                        assert_eq!(PackedBlockPos::try_from(pos), Err(Error(pos)));
                    }
                }
            }
        }
    }
}
