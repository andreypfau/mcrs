use mcrs_minecraft_sound::SoundEventValue;

use crate::RegistryValue;
use crate::SoundEventId;

pub type SoundEventRef = RegistryValue<SoundEventId, SoundEventValue>;
