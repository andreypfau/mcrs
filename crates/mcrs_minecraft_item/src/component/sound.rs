use mcrs_minecraft_core::rl;
use serde::{Deserialize, Serialize};

use crate::component::common::Holder;
use crate::harness::Sample;
use mcrs_minecraft_sound::SoundEvent;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BreakSound(pub Holder<SoundEvent>);

pub fn sound_holder_tags(sound: &Holder<SoundEvent>) -> Vec<(&'static str, u8)> {
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

pub fn sound_holder_samples() -> Vec<Holder<SoundEvent>> {
    vec![
        Holder::Reference(mcrs_minecraft_sound::keys::sound_event::ENTITY_ITEM_BREAK.id()),
        Holder::Direct(SoundEvent {
            sound_id: rl!("mcrs:custom").to_arc(),
            range: None,
        }),
        Holder::Direct(SoundEvent {
            sound_id: rl!("mcrs:custom").to_arc(),
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
