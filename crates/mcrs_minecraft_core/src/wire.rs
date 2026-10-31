use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;

use mcrs_minecraft_wire::{Bounded, Context, Decode, Encode, Error, ErrorKind, Result, VarInt};

use crate::codec::{BoundedString, CompactList};
use crate::{BlockStateNumber, Direction, Id, ResourceKey, ResourceLocation};

impl<S: AsRef<str>> Encode for ResourceLocation<S> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_str().encode(w)
    }
}

impl<'a> Decode<'a> for ResourceLocation<Cow<'a, str>> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        ResourceLocation::read_cow(<Cow<'a, str>>::decode(r)?).map_err(Error::other)
    }
}

impl Decode<'_> for ResourceLocation<Arc<str>> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        ResourceLocation::read(<&str>::decode(r)?).map_err(Error::other)
    }
}

impl<T, S: AsRef<str>> Encode for ResourceKey<T, S> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.location().encode(w)
    }
}

impl<T> Decode<'_> for ResourceKey<T, Arc<str>> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(ResourceKey::from_location(ResourceLocation::decode(r)?))
    }
}

/// An id writes as its sixteen-bit number. Whether a registry holds that number is checked where
/// the id is looked up, since only the registries the connection holds know their size.
impl<R> Encode for Id<R> {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(i32::from(self.number())).encode(w)
    }
}

impl<R> Decode<'_> for Id<R> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let VarInt(raw) = VarInt::decode(r)?;
        u16::try_from(raw)
            .ok()
            .and_then(|number| Id::checked(number, usize::from(u16::MAX) + 1))
            .with_context(|| format!("registry id {raw} is outside 0..=65535"))
    }
}

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
