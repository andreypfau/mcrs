#[rustfmt::skip]
pub mod keys;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::lenient_float;
use mcrs_minecraft_core::registry_key::RegistryValue;
use serde::{Deserialize, Serialize};

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

impl RegistryValue for SoundEvent {
    type Registry = Self;
}
