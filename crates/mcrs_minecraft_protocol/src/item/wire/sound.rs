use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::sound::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode};

newtype_ctx_wire!(BreakSound);

impl EncodeCtx for SoundEvent {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.sound_id.encode(&mut w)?;
        self.range.encode(w)
    }
}

impl DecodeCtx<'_> for SoundEvent {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SoundEvent {
            sound_id: ResourceLocation::decode(r)?,
            range: Option::decode(r)?,
        })
    }
}
