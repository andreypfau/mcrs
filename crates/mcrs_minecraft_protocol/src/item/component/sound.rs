use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Registered, SoundEventReg, lenient, stub_component};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::{Decode, Encode};

/// `SoundEvent.DIRECT_CODEC`; the wire form is `DIRECT_STREAM_CODEC`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundEvent {
    pub sound_id: ResourceLocation,
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Option::is_none"
    )]
    pub range: Option<f32>,
}

impl Registered for SoundEvent {
    type Registry = SoundEventReg;
}

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

stub_component!(BreakSound);
