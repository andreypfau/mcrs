mod block_sound_set;
#[rustfmt::skip]
pub mod keys;

pub use block_sound_set::BlockSoundSet;

use mcrs_minecraft_core::codec::lenient_float;
use mcrs_minecraft_registries::Ref;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_wire::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// A sound event named by its key, or written out in place.
pub type Sound<M> = Ref<M, keys::SoundEvent, SoundEventValue>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SoundEventValue {
    pub sound_id: ResourceLocationBuf,
    #[serde(
        default,
        deserialize_with = "lenient_float",
        skip_serializing_if = "Option::is_none"
    )]
    pub range: Option<f32>,
}

mcrs_minecraft_registries::resolve_as_is!(SoundEventValue);
