use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

/// The bounds of a number argument; a missing bound is the limit of the type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberRange<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: Encode> Encode for NumberRange<T> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let flags = u8::from(self.min.is_some()) | (u8::from(self.max.is_some()) << 1);
        flags.encode(&mut w)?;
        if let Some(min) = &self.min {
            min.encode(&mut w)?;
        }
        if let Some(max) = &self.max {
            max.encode(&mut w)?;
        }
        Ok(())
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for NumberRange<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let flags = u8::decode(r)?;
        let min = (flags & 0x01 != 0).then(|| T::decode(r)).transpose()?;
        let max = (flags & 0x02 != 0).then(|| T::decode(r)).transpose()?;
        Ok(Self { min, max })
    }
}
