use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{self, NonNegativeInt, is_default};
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{
    Holder, HolderWireOnly, InstrumentReg, JukeboxSongReg, Registered,
};
use crate::item::component::consume::{non_negative_float, non_negative_int, positive_float};
use crate::item::component::sound::SoundEvent;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::text::Text;
use crate::{Decode, Encode, VarInt};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentValue {
    pub sound_event: Holder<SoundEvent>,
    #[serde(deserialize_with = "non_negative_float")]
    pub use_duration: f32,
    #[serde(deserialize_with = "positive_float")]
    pub range: f32,
    #[serde(
        default,
        deserialize_with = "non_negative_int",
        skip_serializing_if = "is_default"
    )]
    pub durability_damage: NonNegativeInt,
    pub description: Text,
}

impl Registered for InstrumentValue {
    type Registry = InstrumentReg;
}

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Instrument(pub Holder<InstrumentValue>);

impl EncodeCtx for Instrument {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Instrument {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(Instrument)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JukeboxSong {
    pub sound_event: Holder<SoundEvent>,
    pub description: Text,
    #[serde(deserialize_with = "positive_float")]
    pub length_in_seconds: f32,
    pub comparator_output: codec::Bounded<0, 15>,
}

impl Registered for JukeboxSong {
    type Registry = JukeboxSongReg;
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

/// The persistent form is the registry id alone (`RegistryFixedCodec`); the
/// wire form carries the song inline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JukeboxPlayable(pub HolderWireOnly<JukeboxSong>);

impl EncodeCtx for JukeboxPlayable {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for JukeboxPlayable {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        HolderWireOnly::decode_ctx(ctx, r).map(JukeboxPlayable)
    }
}

impl Sample for Instrument {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID, STRING_ID};
        match &self.0 {
            Holder::Reference(_) => vec![("", STRING_ID)],
            Holder::Direct(value) => {
                let mut tags = vec![
                    ("", COMPOUND_ID),
                    ("use_duration", FLOAT_ID),
                    ("range", FLOAT_ID),
                ];
                if !is_default(&value.durability_damage) {
                    tags.push(("durability_damage", INT_ID));
                }
                tags
            }
        }
    }

    fn samples() -> Vec<Self> {
        vec![
            Instrument(Holder::reference(ResourceLocation::minecraft(
                "ponder_goat_horn",
            ))),
            Instrument(Holder::Direct(InstrumentValue {
                sound_event: Holder::reference(ResourceLocation::minecraft("entity.item.break")),
                use_duration: 7.0,
                range: 256.0,
                durability_damage: codec::Bounded(0),
                description: Text::text("Horn"),
            })),
            Instrument(Holder::Direct(InstrumentValue {
                sound_event: Holder::Direct(SoundEvent {
                    sound_id: ResourceLocation::new("mcrs", "toot"),
                    range: Some(16.0),
                }),
                use_duration: 0.0,
                range: 1.5,
                durability_damage: codec::Bounded(3),
                description: Text::translate("instrument.mcrs.toot", Vec::new()),
            })),
        ]
    }
}

impl Sample for JukeboxPlayable {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![JukeboxPlayable(HolderWireOnly(Holder::reference(
            ResourceLocation::minecraft("pigstep"),
        )))]
    }
}
