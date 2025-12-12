use std::io::{Cursor, Write};

use anyhow::Context;
use mcrs_nbt::Nbt;
use mcrs_nbt::compound::NbtCompound;
use mcrs_nbt::deserializer::NbtReadHelper;
use mcrs_nbt::serializer::WriteAdaptor;
use uuid::Uuid;
use valence_ident::{Ident, IdentError};

use crate::{Decode, Encode, ItemId, VarInt};

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            Some(t) => {
                true.encode(&mut w)?;
                t.encode(w)
            }
            None => false.encode(w),
        }
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Option<T> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => Some(T::decode(r)?),
            false => None,
        })
    }
}

impl Encode for Uuid {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_u128().encode(w)
    }
}

impl<'a> Decode<'a> for Uuid {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        u128::decode(r).map(Uuid::from_u128)
    }
}

impl Encode for NbtCompound {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let mut writer = WriteAdaptor::new(&mut w);
        writer.write_u8_be(mcrs_nbt::COMPOUND_ID)?;
        self.serialize_content(&mut writer)?;
        Ok(())
    }
}

impl Decode<'_> for NbtCompound {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let mut reader = NbtReadHelper::new(Cursor::new(r));
        Nbt::read_unnamed(&mut reader)
            .map(|n| n.root_tag)
            .map_err(|e| e.into())
    }
}

impl<S: Encode> Encode for Ident<S> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_ref().encode(w)
    }
}

impl<'a, S> Decode<'a> for Ident<S>
where
    S: Decode<'a>,
    Ident<S>: TryFrom<S, Error = IdentError>,
{
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Ident::try_from(S::decode(r)?)?)
    }
}

impl Encode for ItemId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0 as i32).encode(w)
    }
}

impl Decode<'_> for ItemId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let errmsg = "invalid item ID";

        Ok(ItemId(id.try_into().context(errmsg)?))
    }
}
