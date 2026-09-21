use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Holder, Registered, SoundEventReg, lenient_float};
use crate::item::harness::Sample;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BreakSound(pub Holder<SoundEvent>);

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
