use crate::item::component::sound::*;
use crate::item::wire::{newtype_wire, record_wire};
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_sound::SoundEventValue;

newtype_wire!(BreakSound<Wire>);
record_wire!(SoundEventValue { sound_id, range });
