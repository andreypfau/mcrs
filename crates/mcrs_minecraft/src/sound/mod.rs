use bevy_asset::{Asset, Handle, uuid_handle};
use bevy_reflect::TypePath;
use mcrs_protocol::Ident;
use mcrs_registry::{Holder, RegistryEntry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Asset, TypePath)]
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

pub const ITEM_BREAK: Handle<SoundEvent> = uuid_handle!("019b30d8-a349-78b7-b7f5-d94ad743dd6d");
