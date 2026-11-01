use bevy::math::DVec2;
use bevy::prelude::*;

/// What the local player asks to do this tick. Whatever drives the player, a keyboard, a script
/// or a bot, writes it before `LocalPlayerTick`.
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

    /// The `(left, forward)` direction of the pressed keys, of length one or zero.
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
