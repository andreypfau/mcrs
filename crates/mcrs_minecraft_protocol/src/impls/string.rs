use std::io::{Cursor, Write};

use crate::text::{Text, TextContent, TextInner};
use crate::{Bounded, Decode, Encode, VarInt};
use anyhow::{Context, ensure};
use byteorder::WriteBytesExt;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{COMPOUND_ID, STRING_ID, from_bytes_unnamed, to_bytes_unnamed};

const DEFAULT_MAX_STRING_CHARS: usize = 32767;

impl Encode for str {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        Bounded::<_, DEFAULT_MAX_STRING_CHARS>(self).encode(w)
    }
}

impl<const MAX_CHARS: usize> Encode for Bounded<&'_ str, MAX_CHARS> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let char_count = self.encode_utf16().count();

        ensure!(
            char_count <= MAX_CHARS,
            "char count of string exceeds maximum (expected <= {MAX_CHARS}, got {char_count})"
        );

        VarInt(self.len() as i32).encode(&mut w)?;
        Ok(w.write_all(self.as_bytes())?)
    }
}

impl<'a> Decode<'a> for &'a str {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Bounded::<_, DEFAULT_MAX_STRING_CHARS>::decode(r)?.0)
    }
}

impl<'a, const MAX_CHARS: usize> Decode<'a> for Bounded<&'a str, MAX_CHARS> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode string with negative length");
        let len = len as usize;
        ensure!(
            len <= r.len(),
            "not enough data remaining ({} bytes) to decode string of {len} bytes",
            r.len()
        );

        let (res, remaining) = r.split_at(len);
        let res = std::str::from_utf8(res)?;

        let char_count = res.encode_utf16().count();
        ensure!(
            char_count <= MAX_CHARS,
            "char count of string exceeds maximum (expected <= {MAX_CHARS}, got {char_count})"
        );

        *r = remaining;

        Ok(Bounded(res))
    }
}

impl Encode for String {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_str().encode(w)
    }
}

impl<const MAX_CHARS: usize> Encode for Bounded<String, MAX_CHARS> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        Bounded::<_, MAX_CHARS>(self.as_str()).encode(w)
    }
}

impl Decode<'_> for String {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(<&str>::decode(r)?.into())
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for Bounded<String, MAX_CHARS> {
    fn decode(r: &mut &'_ [u8]) -> anyhow::Result<Self> {
        Ok(Bounded(Bounded::<&str, MAX_CHARS>::decode(r)?.0.into()))
    }
}

impl Decode<'_> for Box<str> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(<&str>::decode(r)?.into())
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for Bounded<Box<str>, MAX_CHARS> {
    fn decode(r: &mut &'_ [u8]) -> anyhow::Result<Self> {
        Ok(Bounded(Bounded::<&str, MAX_CHARS>::decode(r)?.0.into()))
    }
}

impl Encode for Text {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        if let TextContent::Text { text } = &self.content
            && self.extra.is_empty()
            && self.color.is_none()
            && self.font.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underlined.is_none()
            && self.strikethrough.is_none()
            && self.obfuscated.is_none()
            && self.click_event.is_none()
            && self.hover_event.is_none()
            && self.insertion.is_none()
        {
            w.write_u8(STRING_ID)?;
            NbtTag::String(text.to_string()).serialize_data(&mut WriteAdaptor::new(w))?;
            return Ok(());
        }
        to_bytes_unnamed(&self, &mut w)?;
        Ok(())
    }
}

impl Decode<'_> for Text {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let tag = *r.first().context("empty input for Text")?;
        let mut cursor = Cursor::new(*r);
        let text = match tag {
            STRING_ID => match NbtTag::deserialize(&mut NbtReadHelper::new(&mut cursor))? {
                NbtTag::String(s) => Self::text(s),
                other => anyhow::bail!("expected an NBT string tag for Text, got {other:?}"),
            },
            COMPOUND_ID => Self::from(from_bytes_unnamed::<TextInner>(&mut cursor)?),
            other => anyhow::bail!("expected an NBT string or compound tag for Text, got {other}"),
        };
        *r = &r[cursor.position() as usize..];
        Ok(text)
    }
}
