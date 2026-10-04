use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{self, NonNegativeInt, is_default};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::{Holder, HolderWireOnly, InstrumentReg, JukeboxSongReg, Registered};
use crate::component::consume::{non_negative_float, positive_float};
use crate::component::sound::SoundEvent;
use crate::harness::Sample;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentValue {
    pub sound_event: Holder<SoundEvent>,
    #[serde(deserialize_with = "non_negative_float")]
    pub use_duration: f32,
    #[serde(deserialize_with = "positive_float")]
    pub range: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub durability_damage: NonNegativeInt,
    pub description: Text,
}

impl Registered for InstrumentValue {
    type Registry = InstrumentReg;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Instrument(pub Holder<InstrumentValue>);

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

/// The persistent form is the registry id alone; the wire form carries the
/// song inline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JukeboxPlayable(pub HolderWireOnly<JukeboxSong>);

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
