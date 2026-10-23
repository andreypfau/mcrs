use crate::item::component::sound::*;
use crate::item::wire::{newtype_ctx_wire, record_ctx_wire};
use mcrs_minecraft_sound::SoundEventValue;

newtype_ctx_wire!(BreakSound);
record_ctx_wire!(SoundEventValue { sound_id, range });
