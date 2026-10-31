// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod block_sound_set;
pub mod block_sound_set_tags;
pub mod sound_event;

pub use sound_event::SoundEvent;

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_SOUND_SET: RegistryKey<crate::BlockSoundSet> =
    RegistryKey::new(rl!("minecraft:block_sound_set"));
impl Registered for crate::BlockSoundSet {
    const REGISTRY: RegistryKey<Self> = BLOCK_SOUND_SET;
}

pub const SOUND_EVENT: RegistryKey<crate::keys::SoundEvent> =
    RegistryKey::new(rl!("minecraft:sound_event"));
impl Registered for crate::keys::SoundEvent {
    const REGISTRY: RegistryKey<Self> = SOUND_EVENT;
}
