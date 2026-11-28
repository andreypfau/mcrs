use bevy::math::DVec2;
use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MovementInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

impl MovementInput {
    pub const EMPTY: Self = Self {
        forward: false,
        backward: false,
        left: false,
        right: false,
        jump: false,
        shift: false,
        sprint: false,
    };

    #[must_use]
    pub fn or(self, other: MovementInput) -> MovementInput {
        MovementInput {
            forward: self.forward || other.forward,
            backward: self.backward || other.backward,
            left: self.left || other.left,
            right: self.right || other.right,
            jump: self.jump || other.jump,
            shift: self.shift || other.shift,
            sprint: self.sprint || other.sprint,
        }
    }

    pub fn move_vector(self) -> DVec2 {
        DVec2::new(
            impulse(self.left, self.right),
            impulse(self.forward, self.backward),
        )
        .normalize_or_zero()
    }
}

fn impulse(positive: bool, negative: bool) -> f64 {
    f64::from(i8::from(positive) - i8::from(negative))
}
