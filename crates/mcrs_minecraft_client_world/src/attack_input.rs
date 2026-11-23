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

    #[must_use]
    pub fn with(self, scripted: AttackInput) -> AttackInput {
        if self.screen_open {
            return self;
        }
        AttackInput {
            presses: self.presses.saturating_add(scripted.presses),
            down: self.down || scripted.down,
            screen_open: false,
        }
    }
}
