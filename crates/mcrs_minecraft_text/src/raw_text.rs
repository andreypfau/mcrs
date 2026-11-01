use std::borrow::Cow;
use std::io::Write;

use mcrs_minecraft_nbt::{COMPOUND_ID, LIST_ID, STRING_ID, split_wire};
use mcrs_minecraft_wire::{Decode, Encode, Error, Result};

use crate::{ItemHover, Text, TextSerde};

/// The network bytes of one text component, checked to be one well-formed NBT tag of a type
/// that a text takes. A packet that only forwards a text keeps it in this form; a reader that
/// needs the text calls [`RawText::to_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawText<'a>(Cow<'a, [u8]>);

impl RawText<'static> {
    /// Encodes `text` once, so that every packet that carries the result writes the same bytes.
    pub fn from_text<I: TextSerde>(text: &Text<I>) -> Result<RawText<'static>> {
        let mut bytes = Vec::new();
        text.encode(&mut bytes)?;
        Ok(RawText(Cow::Owned(bytes)))
    }
}

impl RawText<'_> {
    pub fn to_text(&self) -> Result<Text<ItemHover>> {
        let mut r = self.as_bytes();
        let text = Text::decode(&mut r)?;
        if !r.is_empty() {
            return Err(Error::invalid("a raw text holds bytes past its tag"));
        }
        Ok(text)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_owned(self) -> RawText<'static> {
        RawText(Cow::Owned(self.0.into_owned()))
    }
}

impl Encode for RawText<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        w.write_all(&self.0)?;
        Ok(())
    }
}

impl<'a> Decode<'a> for RawText<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        if !matches!(r.first(), Some(&(STRING_ID | COMPOUND_ID | LIST_ID)) | None) {
            return Err(Error::invalid(
                "a text is a string, a compound or a list tag",
            ));
        }
        Ok(RawText(Cow::Borrowed(split_wire(r)?)))
    }
}
