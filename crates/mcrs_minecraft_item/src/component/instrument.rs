use std::sync::Arc;

use mcrs_minecraft_core::codec::{self, NonNegativeInt, is_default};
use mcrs_minecraft_core::rl;
use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::WireRef;
use crate::component::consume::{non_negative_float, positive_float};
use crate::harness::Sample;
use crate::harness::entry;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_sound::{Sound, SoundEventValue};
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt};
use std::io::Write;

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentValue<M: Refs = Resolved> {
    pub sound_event: Sound<M>,
    #[serde(deserialize_with = "non_negative_float")]
    pub use_duration: f32,
    #[serde(deserialize_with = "positive_float")]
    pub range: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub durability_damage: NonNegativeInt,
    pub description: Text<M::Nbt>,
}

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(transparent)]
pub struct Instrument<M: Refs = Resolved>(pub Ref<M, InstrumentValue, InstrumentValue<M>>);

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JukeboxSong<M: Refs = Resolved> {
    pub sound_event: Sound<M>,
    pub description: Text<M::Nbt>,
    #[serde(deserialize_with = "positive_float")]
    pub length_in_seconds: f32,
    pub comparator_output: codec::Bounded<0, 15>,
}

/// The persistent form is the registry id alone; the wire form carries the
/// song inline.
#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(transparent)]
pub struct JukeboxPlayable<M: Refs = Resolved>(pub WireRef<M, JukeboxSong, JukeboxSong<M>>);

impl Sample for Instrument {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID, STRING_ID};
        match &self.0 {
            Ref::Id(_) => vec![("", STRING_ID)],
            Ref::Inline(value) => {
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
            Instrument(Ref::Id(entry("ponder_goat_horn"))),
            Instrument(Ref::Inline(Arc::new(InstrumentValue {
                sound_event: Ref::Id(mcrs_minecraft_sound::keys::SoundEvent::EntityItemBreak.id()),
                use_duration: 7.0,
                range: 256.0,
                durability_damage: codec::Bounded(0),
                description: Text::text("Horn"),
            }))),
            Instrument(Ref::Inline(Arc::new(InstrumentValue {
                sound_event: Ref::Inline(Arc::new(SoundEventValue {
                    sound_id: rl!("mcrs:toot").to_arc(),
                    range: Some(16.0),
                })),
                use_duration: 0.0,
                range: 1.5,
                durability_damage: codec::Bounded(3),
                description: Text::translate("instrument.mcrs.toot", Vec::new()),
            }))),
        ]
    }
}

impl Sample for JukeboxPlayable {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![JukeboxPlayable(WireRef(Ref::Id(entry("pigstep"))))]
    }
}

impl Encode for InstrumentValue<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.sound_event.encode(&mut w)?;
        self.use_duration.encode(&mut w)?;
        self.range.encode(&mut w)?;
        VarInt(self.durability_damage.0).encode(&mut w)?;
        self.description.encode(w)
    }
}

impl Decode<'_> for InstrumentValue<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(InstrumentValue {
            sound_event: Decode::decode(r)?,
            use_duration: f32::decode(r)?,
            range: f32::decode(r)?,
            durability_damage: codec::Bounded(VarInt::decode(r)?.0),
            description: Decode::decode(r)?,
        })
    }
}

impl Encode for JukeboxSong<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.sound_event.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.length_in_seconds.encode(&mut w)?;
        VarInt(self.comparator_output.0).encode(w)
    }
}

impl Decode<'_> for JukeboxSong<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(JukeboxSong {
            sound_event: Decode::decode(r)?,
            description: Decode::decode(r)?,
            length_in_seconds: f32::decode(r)?,
            comparator_output: codec::Bounded(VarInt::decode(r)?.0),
        })
    }
}
