use core::hash::Hash;
use core::hash::Hasher;

use crate::float_key;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Food {
    pub nutrition: i32,
    pub saturation: f32,
    pub can_always_eat: bool,
}

impl Food {
    #[must_use]
    pub const fn new(nutrition: i32, saturation: f32) -> Option<Self> {
        if nutrition >= 0 {
            Some(Self {
                nutrition,
                saturation,
                can_always_eat: false,
            })
        } else {
            None
        }
    }
}

impl PartialEq for Food {
    fn eq(&self, other: &Self) -> bool {
        self.nutrition == other.nutrition
            && float_key::of_f32(self.saturation) == float_key::of_f32(other.saturation)
            && self.can_always_eat == other.can_always_eat
    }
}

impl Eq for Food {}

impl Hash for Food {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.nutrition.hash(state);
        float_key::of_f32(self.saturation).hash(state);
        self.can_always_eat.hash(state);
    }
}
