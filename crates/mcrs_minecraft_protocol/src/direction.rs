use crate::{Decode, Encode, VarInt};
use anyhow::bail;
use std::io::Write;

pub use mcrs_voxel_math::Direction;

impl Encode for Direction {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.id() as i32).encode(w)
    }
}

impl Decode<'_> for Direction {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        Ok(match id {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::West,
            5 => Direction::East,
            _ => bail!("invalid direction ID: {id}"),
        })
    }
}
