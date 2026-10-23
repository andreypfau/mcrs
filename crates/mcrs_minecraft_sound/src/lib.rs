mod block_sound_set;
#[rustfmt::skip]
pub mod keys;

pub use block_sound_set::BlockSoundSet;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::lenient_float;
use mcrs_minecraft_registries::Ref;
use serde::{Deserialize, Serialize};

/// A sound event named by its key, or written out in place.
pub type Sound<M> = Ref<M, SoundEvent, SoundEvent>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundEvent {
    pub sound_id: ResourceLocation,
    #[serde(
        default,
        deserialize_with = "lenient_float",
        skip_serializing_if = "Option::is_none"
    )]
    pub range: Option<f32>,
}

mcrs_minecraft_registries::resolve_as_is!(SoundEvent);
