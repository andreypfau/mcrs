use std::io::Write;

use mcrs_minecraft_core::codec;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::common::Holder;
use crate::item::component::instrument::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::text::Text;
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(Instrument, JukeboxPlayable);

impl EncodeCtx for InstrumentValue {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.sound_event.encode_ctx(ctx, &mut w)?;
        self.use_duration.encode(&mut w)?;
        self.range.encode(&mut w)?;
        VarInt(self.durability_damage.0).encode(&mut w)?;
        self.description.encode(w)
    }
}

impl DecodeCtx<'_> for InstrumentValue {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(InstrumentValue {
            sound_event: Holder::decode_ctx(ctx, r)?,
            use_duration: f32::decode(r)?,
            range: f32::decode(r)?,
            durability_damage: codec::Bounded(VarInt::decode(r)?.0),
            description: Text::decode(r)?,
        })
    }
}

impl EncodeCtx for JukeboxSong {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.sound_event.encode_ctx(ctx, &mut w)?;
        self.description.encode(&mut w)?;
        self.length_in_seconds.encode(&mut w)?;
        VarInt(self.comparator_output.0).encode(w)
    }
}

impl DecodeCtx<'_> for JukeboxSong {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(JukeboxSong {
            sound_event: Holder::decode_ctx(ctx, r)?,
            description: Text::decode(r)?,
            length_in_seconds: f32::decode(r)?,
            comparator_output: codec::Bounded(VarInt::decode(r)?.0),
        })
    }
}
