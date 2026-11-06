use bevy::prelude::*;

/// The attack input as the window last reported it, for the next tick to take.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttackInput {
    /// The presses since the last tick took them.
    pub presses: u32,
    /// The input is down, the window holds the mouse, and no screen is open.
    pub down: bool,
    /// A screen is open, or the window does not hold the mouse.
    pub screen_open: bool,
}

impl AttackInput {
    /// Hands the presses to a tick, and keeps the state of the input.
    pub fn take(&mut self) -> AttackInput {
        let taken = *self;
        self.presses = 0;
        taken
    }
}
