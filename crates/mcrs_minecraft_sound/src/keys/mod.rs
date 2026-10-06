// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod sound_event;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const SOUND_EVENT: RegistryKey<crate::SoundEvent> = RegistryKey::new(rl!("minecraft:sound_event"));
impl Registered for crate::SoundEvent {
    const REGISTRY: RegistryKey<Self> = SOUND_EVENT;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        SOUND_EVENT.binding(),
    ]
}
