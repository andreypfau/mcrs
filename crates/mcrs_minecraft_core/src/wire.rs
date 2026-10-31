use std::io::Write;

use mcrs_minecraft_wire::{Bounded, Context, Decode, Encode, ErrorKind, Result, VarInt};

use crate::codec::{BoundedString, CompactList};
use crate::{BlockStateNumber, Direction};

impl Encode for BlockStateNumber {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(i32::from(self.0)).encode(w)
    }
}

impl Decode<'_> for BlockStateNumber {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let VarInt(raw) = VarInt::decode(r)?;
        u16::try_from(raw)
            .map(BlockStateNumber)
            .ok()
            .with_context(|| format!("block state {raw} is outside 0..=65535"))
    }
}

impl<const MAX_CHARS: usize> Encode for BoundedString<MAX_CHARS> {
    fn encode(&self, w: impl Write) -> Result<()> {
        Bounded::<&str, MAX_CHARS>(&self.0).encode(w)
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for BoundedString<MAX_CHARS> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(BoundedString(
            Bounded::<&str, MAX_CHARS>::decode(r)?.0.into(),
        ))
    }
}

impl<T: Encode> Encode for CompactList<T> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.0.encode(w)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for CompactList<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Vec::decode(r).map(CompactList)
    }
}

impl Encode for Direction {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(self.id() as i32).encode(w)
    }
}

impl Decode<'_> for Direction {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::West,
            5 => Direction::East,
            tag => {
                let ty = "Direction";
                return Err(ErrorKind::UnknownTag { ty, tag }.into());
            }
        })
    }
}
