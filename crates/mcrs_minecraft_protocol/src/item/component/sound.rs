use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Holder, Registered, SoundEventReg, lenient_float};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode};

/// `SoundEvent.DIRECT_CODEC`; the wire form is `DIRECT_STREAM_CODEC`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundEvent {
    pub sound_id: ResourceLocation,
    #[serde(
        default,
        deserialize_with = "lenient_float",
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BreakSound(pub Holder<SoundEvent>);

impl EncodeCtx for BreakSound {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BreakSound {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(BreakSound)
    }
}

pub(crate) fn sound_holder_tags(sound: &Holder<SoundEvent>) -> Vec<(&'static str, u8)> {
    use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, STRING_ID};
    match sound {
        Holder::Reference(_) => vec![("", STRING_ID)],
        Holder::Direct(event) => {
            let mut tags = vec![("", COMPOUND_ID), ("sound_id", STRING_ID)];
            if event.range.is_some() {
                tags.push(("range", FLOAT_ID));
            }
            tags
        }
    }
}

pub(crate) fn sound_holder_samples() -> Vec<Holder<SoundEvent>> {
    vec![
        Holder::reference(ResourceLocation::minecraft("entity.item.break")),
        Holder::Direct(SoundEvent {
            sound_id: ResourceLocation::new("mcrs", "custom"),
            range: None,
        }),
        Holder::Direct(SoundEvent {
            sound_id: ResourceLocation::new("mcrs", "custom"),
            range: Some(12.5),
        }),
    ]
}

impl Sample for BreakSound {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        sound_holder_tags(&self.0)
    }

    fn samples() -> Vec<Self> {
        sound_holder_samples().into_iter().map(BreakSound).collect()
    }
}
