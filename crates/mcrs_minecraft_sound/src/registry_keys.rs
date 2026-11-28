use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK_SOUND_SET: RegistryKey<crate::BlockSoundSet> =
    <crate::BlockSoundSet as Registered>::REGISTRY;
impl Registered for crate::BlockSoundSet {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:block_sound_set"));
}

pub const SOUND_EVENT: RegistryKey<crate::sound_event_keys::SoundEvent> =
    <crate::sound_event_keys::SoundEvent as Registered>::REGISTRY;
impl Registered for crate::sound_event_keys::SoundEvent {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:sound_event"));
}
