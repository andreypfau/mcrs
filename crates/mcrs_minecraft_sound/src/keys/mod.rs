// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod block_sound_set;
pub mod block_sound_set_tags;
pub mod sound_event;

use mcrs_minecraft_core::{Registered, RegistryKey, TypeBinding, rl};

pub const BLOCK_SOUND_SET: RegistryKey<crate::BlockSoundSet> =
    RegistryKey::new(rl!("minecraft:block_sound_set"));
impl Registered for crate::BlockSoundSet {
    const REGISTRY: RegistryKey<Self> = BLOCK_SOUND_SET;
}

pub const SOUND_EVENT: RegistryKey<crate::SoundEvent> =
    RegistryKey::new(rl!("minecraft:sound_event"));
impl Registered for crate::SoundEvent {
    const REGISTRY: RegistryKey<Self> = SOUND_EVENT;
}

pub fn bindings() -> [TypeBinding; 2] {
    [BLOCK_SOUND_SET.binding(), SOUND_EVENT.binding()]
}
