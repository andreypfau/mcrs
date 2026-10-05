use std::borrow::Cow;
use std::io::{Cursor, Write};
use std::sync::Arc;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::Nbt;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use uuid::Uuid;

use crate::registry::{decode_registry_id, encode_registry_id};
use crate::{Decode, Encode};
use mcrs_minecraft_registry::ItemId;

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
        writer.write_u8_be(mcrs_minecraft_nbt::COMPOUND_ID)?;
        self.serialize_content(&mut writer)?;
        Ok(())
    }
}

impl Decode<'_> for NbtCompound {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(*r);
        let nbt = Nbt::read_unnamed(&mut NbtReadHelper::new(&mut cursor))?;
        *r = &r[cursor.position() as usize..];
        Ok(nbt.root_tag)
    }
}

impl Encode for NbtTag {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.serialize(&mut WriteAdaptor::new(&mut w))?;
        Ok(())
    }
}

impl Decode<'_> for NbtTag {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(*r);
        let tag = NbtTag::deserialize(&mut NbtReadHelper::new(&mut cursor))?;
        *r = &r[cursor.position() as usize..];
        Ok(tag)
    }
}

impl<S: AsRef<str>> Encode for ResourceLocation<S> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_str().encode(w)
    }
}

impl<'a> Decode<'a> for ResourceLocation<Cow<'a, str>> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(ResourceLocation::parse_cow(<Cow<'a, str>>::decode(r)?)?)
    }
}

impl Decode<'_> for ResourceLocation<Arc<str>> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ResourceLocation::parse(<&str>::decode(r)?)?)
    }
}

impl<T, S: AsRef<str>> Encode for ResourceKey<T, S> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.location().encode(w)
    }
}

impl<T> Decode<'_> for ResourceKey<T, Arc<str>> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ResourceKey::from_location(ResourceLocation::decode(r)?))
    }
}

impl Encode for ItemId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.0, w)
    }
}

impl Decode<'_> for ItemId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_registry_id(r).map(ItemId)
    }
}
