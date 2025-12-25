use crate::{Decode, Encode, VarInt, nbt};
use mcrs_nbt::compound::NbtCompound;
use std::borrow::Cow;
use std::io::Write;
use valence_ident::Ident;

#[derive(Clone, Debug, Encode, Decode)]
pub struct Entry<'a> {
    pub id: Ident<Cow<'a, str>>,
    pub data: Option<Cow<'a, NbtCompound>>,
}

#[derive(Clone, Debug)]
pub enum Holder {
    Reference(i32),
    Direct(NbtCompound),
}

impl Encode for Holder {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            Holder::Reference(id) => {
                VarInt(*id + 1).encode(&mut w)?;
            }
            Holder::Direct(compound) => {
                VarInt(0).encode(&mut w)?;
                nbt::to_bytes_unnamed(compound, &mut w)?;
            }
        }
        Ok(())
    }
}

impl<'a> Decode<'a> for Holder {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let i = VarInt::decode(r)?;
        if i.0 == 0 {
            let cursor = std::io::Cursor::new(&r[i.written_size()..]);
            let compound = nbt::from_bytes_unnamed(cursor)?;
            Ok(Holder::Direct(compound))
        } else {
            Ok(Holder::Reference(i.0 - 1))
        }
    }
}
