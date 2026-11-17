use crate::InlineSoundEvent;
use crate::RegistryValue;
use crate::SoundEventId;

pub type SoundEventRef = RegistryValue<SoundEventId, InlineSoundEvent>;
