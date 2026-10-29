use crate::{Decode, Encode, Error, Result, VarInt};
use std::io::Write;

/// A chat type on the wire: an entry of the synced registry `minecraft:chat_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatTypeId(pub u32);

impl Encode for ChatTypeId {
    fn encode(&self, w: impl Write) -> Result<()> {
        let value = i32::try_from(self.0.saturating_add(1))
            .map_err(|_| Error::invalid("chat type id out of range"))?;
        VarInt(value).encode(w)
    }
}

impl<'a> Decode<'a> for ChatTypeId {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        match VarInt::decode(r)?.0 {
            value if value > 0 => Ok(Self(value.unsigned_abs() - 1)),
            0 => Err(Error::invalid(
                "an inline chat type has no specified network form",
            )),
            other => Err(Error::invalid(format!("chat type {other} is negative"))),
        }
    }
}

/// A signature of the last seen list of a `player_chat`: a full signature, or an entry of the
/// signature cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackedSignature<'a> {
    /// The full signature.
    Full(&'a [u8; 256]),
    /// The index of the signature in the cache.
    Cached(u32),
}

impl Encode for PackedSignature<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            PackedSignature::Full(signature) => {
                VarInt(0).encode(&mut w)?;
                signature.encode(w)
            }
            PackedSignature::Cached(id) => {
                let value = i32::try_from(id.saturating_add(1))
                    .map_err(|_| Error::invalid("signature cache id out of range"))?;
                VarInt(value).encode(w)
            }
        }
    }
}

impl<'a> Decode<'a> for PackedSignature<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        match VarInt::decode(r)?.0 {
            0 => Ok(PackedSignature::Full(Decode::decode(r)?)),
            value if value > 0 => Ok(PackedSignature::Cached(value.unsigned_abs() - 1)),
            other => Err(Error::invalid(format!(
                "signature cache id {other} is negative"
            ))),
        }
    }
}

/// Which code units of a `player_chat` body the server hid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterMask {
    /// The server hid nothing.
    PassThrough,
    /// The server hid the whole body.
    FullyFiltered,
    /// Bit `i` is bit `i mod 8` of byte `i / 8`.
    PartiallyFiltered(Vec<u8>),
}

impl FilterMask {
    /// Whether the server hid the code unit at `index`.
    pub fn hides(&self, index: usize) -> bool {
        match self {
            FilterMask::PassThrough => false,
            FilterMask::FullyFiltered => true,
            FilterMask::PartiallyFiltered(bits) => bits
                .get(index / 8)
                .is_some_and(|byte| byte & (1 << (index % 8)) != 0),
        }
    }
}

impl Encode for FilterMask {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            FilterMask::PassThrough => VarInt(0).encode(w),
            FilterMask::FullyFiltered => VarInt(1).encode(w),
            FilterMask::PartiallyFiltered(bits) => {
                VarInt(2).encode(&mut w)?;
                bits.encode(w)
            }
        }
    }
}

impl<'a> Decode<'a> for FilterMask {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        match VarInt::decode(r)?.0 {
            0 => Ok(FilterMask::PassThrough),
            1 => Ok(FilterMask::FullyFiltered),
            2 => Ok(FilterMask::PartiallyFiltered(Decode::decode(r)?)),
            other => Err(Error::invalid(format!(
                "filter type {other} does not exist"
            ))),
        }
    }
}

/// What a `custom_chat_completions` packet does to the custom completion set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum CompletionAction {
    /// Adds the entries to the set.
    Add,
    /// Removes the entries from the set.
    Remove,
    /// Replaces the set with the entries.
    Set,
}
