use std::sync::Arc;

use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_sound::{Sound, SoundEventValue};
use serde::{Deserialize, Serialize};

use crate::harness::Sample;
use mcrs_minecraft_wire::{Decode, Encode};

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
pub struct BreakSound<M: Refs = Resolved>(pub Sound<M>);

pub fn sound_holder_tags(sound: &Sound<Resolved>) -> Vec<(&'static str, u8)> {
    use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, STRING_ID};
    match sound {
        Ref::Id(_) => vec![("", STRING_ID)],
        Ref::Inline(event) => {
            let mut tags = vec![("", COMPOUND_ID), ("sound_id", STRING_ID)];
            if event.range.is_some() {
                tags.push(("range", FLOAT_ID));
            }
            tags
        }
    }
}

pub fn sound_holder_samples() -> Vec<Sound<Resolved>> {
    vec![
        Ref::Id(mcrs_minecraft_sound::keys::SoundEvent::EntityItemBreak.id()),
        Ref::Inline(Arc::new(SoundEventValue {
            sound_id: rl!("mcrs:custom").to_arc(),
            range: None,
        })),
        Ref::Inline(Arc::new(SoundEventValue {
            sound_id: rl!("mcrs:custom").to_arc(),
            range: Some(12.5),
        })),
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
