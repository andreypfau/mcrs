mod block_sound_set;
#[rustfmt::skip]
pub mod keys;

pub use block_sound_set::BlockSoundSet;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::lenient_float;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_registries::{Refs, RegistryType};
use serde::{Deserialize, Serialize};

/// A sound event named by its key, or written out in place.
pub type Sound<M> = <M as Refs>::Ref<SoundEvent, SoundEvent>;

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

impl RegistryValue for SoundEvent {
    type Registry = Self;
}

impl RegistryType for SoundEvent {
    type Value = ();
}

mcrs_minecraft_registries::resolve_as_is!(SoundEvent);

mcrs_minecraft_registries::declare_registries!(
    crate::BlockSoundSet => crate::keys::BLOCK_SOUND_SET, World;
);
