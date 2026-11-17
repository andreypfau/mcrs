use bevy::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttackInput {
    pub presses: u32,
    pub down: bool,
    pub screen_open: bool,
}

impl AttackInput {
    pub fn take(&mut self) -> AttackInput {
        let taken = *self;
        self.presses = 0;
        taken
    }
}
