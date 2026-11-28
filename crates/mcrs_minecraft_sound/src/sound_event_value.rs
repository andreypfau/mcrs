use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_core::codec::lenient_float;
#[cfg(feature = "registry")]
use mcrs_minecraft_registries::Ref;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_wire::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[cfg(feature = "registry")]
use crate::sound_event_keys;

#[cfg(feature = "registry")]
pub type Sound<M> = Ref<M, sound_event_keys::SoundEvent, SoundEventValue>;

#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct SoundEventValue {
    pub sound_id: ResourceLocationBuf,
    #[serde(
        default,
        deserialize_with = "lenient_float",
        skip_serializing_if = "Option::is_none"
    )]
    pub range: Option<f32>,
}

impl SoundEventValue {
    fn range_key(&self) -> Option<u32> {
        self.range.map(|range| {
            if range.is_nan() {
                f32::NAN.to_bits()
            } else {
                range.to_bits()
            }
        })
    }
}

impl PartialEq for SoundEventValue {
    fn eq(&self, other: &Self) -> bool {
        self.sound_id == other.sound_id && self.range_key() == other.range_key()
    }
}

impl Eq for SoundEventValue {}

impl Hash for SoundEventValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sound_id.hash(state);
        self.range_key().hash(state);
    }
}

#[cfg(feature = "registry")]
mcrs_minecraft_registries::resolve_as_is!(SoundEventValue);
