use std::io::Write;

use mcrs_minecraft_wire::{Decode, Encode, Result};

use crate::{Text, TextSerde};

impl<I: TextSerde> Encode for Text<I> {
    fn encode(&self, w: impl Write) -> Result<()> {
        mcrs_minecraft_nbt::encode_wire(self, w)
    }
}

impl<I: TextSerde> Decode<'_> for Text<I> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        mcrs_minecraft_nbt::decode_wire(r)
    }
}
