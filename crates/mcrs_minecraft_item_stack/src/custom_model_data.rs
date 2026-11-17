use alloc::string::String;
use alloc::vec::Vec;
use core::hash::Hash;
use core::hash::Hasher;

use crate::ArgbColor;
use crate::float_key;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct CustomModelData {
    pub floats: Vec<f32>,
    pub flags: Vec<bool>,
    pub strings: Vec<String>,
    pub colors: Vec<ArgbColor>,
}

impl PartialEq for CustomModelData {
    fn eq(&self, other: &Self) -> bool {
        self.floats.len() == other.floats.len()
            && (self.floats.iter())
                .zip(&other.floats)
                .all(|(left, right)| float_key::of_f32(*left) == float_key::of_f32(*right))
            && self.flags == other.flags
            && self.strings == other.strings
            && self.colors == other.colors
    }
}

impl Eq for CustomModelData {}

impl Hash for CustomModelData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.floats.len().hash(state);
        for value in &self.floats {
            float_key::of_f32(*value).hash(state);
        }
        self.flags.hash(state);
        self.strings.hash(state);
        self.colors.hash(state);
    }
}
