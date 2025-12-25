use mcrs_protocol::Ident;
use mcrs_registry::{Holder, RegistryEntry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundEvent {
    sound_id: Ident<String>,
    range: f32,
}

impl RegistryEntry for SoundEvent {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Music {
    sound: Holder<SoundEvent>,
    min_delay: i32,
    max_delay: i32,
    replace_current_music: bool,
}
