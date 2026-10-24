use mcrs_minecraft_registries::Wire;
use std::io::Write;

use crate::item::component::enums::DyeColor;
use crate::item::component::text::*;
use crate::item::wire::{newtype_wire, record_wire};
use crate::{Decode, Encode};

newtype_wire!(ItemModel, TooltipStyle, NoteBlockSound);
newtype_wire!(
    CustomName<Wire>,
    ItemName<Wire>,
    SignTextFront<Wire>,
    SignTextBack<Wire>
);

record_wire!(Lore<Wire> { lines });

impl Encode for SignText<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.messages.encode(&mut w)?;
        self.filtered_messages.encode(&mut w)?;
        self.color.encode(&mut w)?;
        self.has_glowing_text.encode(w)
    }
}

impl Decode<'_> for SignText<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SignText::new(
            Decode::decode(r)?,
            Option::decode(r)?,
            DyeColor::decode(r)?,
            bool::decode(r)?,
        ))
    }
}
