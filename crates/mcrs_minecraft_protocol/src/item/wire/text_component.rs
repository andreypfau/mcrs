use std::io::Write;

use crate::item::lookup::{decode_nbt_wire, encode_nbt_wire};
use crate::{Decode, Encode};
use mcrs_minecraft_text::{Text, TextSerde};

impl<I: TextSerde> Encode for Text<I> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_nbt_wire(self, w)
    }
}

impl<I: TextSerde> Decode<'_> for Text<I> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_nbt_wire(r)
    }
}
