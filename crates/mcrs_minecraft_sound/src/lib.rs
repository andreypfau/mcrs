mod block_sound_set;
#[rustfmt::skip]
pub mod keys;

pub use block_sound_set::BlockSoundSet;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::lenient_float;
use mcrs_minecraft_core::registry_key::RegistryValue;
use serde::{Deserialize, Serialize};

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

mcrs_minecraft_registries::read_with_serde!(crate::BlockSoundSet);

mcrs_minecraft_registries::declare_registries!(
    crate::BlockSoundSet => crate::keys::BLOCK_SOUND_SET, World;
);
