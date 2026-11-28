use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_text::Text;

use crate::SoundEventRef;
use crate::float_key;

#[derive(Clone, Debug)]
pub struct InlineInstrument {
    pub sound_event: SoundEventRef,
    pub use_duration: f32,
    pub range: f32,
    pub durability_damage: i32,
    pub description: Text,
}

impl PartialEq for InlineInstrument {
    fn eq(&self, other: &Self) -> bool {
        self.sound_event == other.sound_event
            && float_key::of_f32(self.use_duration) == float_key::of_f32(other.use_duration)
            && float_key::of_f32(self.range) == float_key::of_f32(other.range)
            && self.durability_damage == other.durability_damage
            && self.description == other.description
    }
}

impl Hash for InlineInstrument {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sound_event.hash(state);
        float_key::of_f32(self.use_duration).hash(state);
        float_key::of_f32(self.range).hash(state);
        self.durability_damage.hash(state);
    }
}
