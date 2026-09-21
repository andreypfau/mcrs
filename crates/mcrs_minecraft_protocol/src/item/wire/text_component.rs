use std::io::Write;

use crate::item::ctx::{decode_nbt_wire, encode_nbt_wire};
use crate::text::{HoverItem, TextComponent};
use crate::{Decode, Encode};

impl<I: HoverItem> Encode for TextComponent<I> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_nbt_wire(self, w)
    }
}

impl<I: HoverItem> Decode<'_> for TextComponent<I> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_nbt_wire(r)
    }
}
