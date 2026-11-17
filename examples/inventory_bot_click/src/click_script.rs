use bevy_ecs::resource::Resource;

use crate::ClickStep;

/// Where the mouse button of the current step is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ButtonState {
    #[default]
    Up,
    /// The press sent a click, so the release that follows sends none.
    DownAfterClick,
    /// The press sent no click, so the release may send one.
    DownWithoutClick,
}

/// The progress of the fixed script. Insert it to start the script; the bot then sends
/// one mouse gesture each frame, once its inventory menu has a state id from the
/// server.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickScript {
    next: usize,
    button: ButtonState,
}

impl ClickScript {
    /// Entry 36 is the first hotbar slot and entries 9 to 35 are the main inventory.
    pub const STEPS: &[ClickStep] = &[
        // Pick up the stack of the first hotbar slot.
        ClickStep::left(36),
        // Put it down in the first main inventory slot.
        ClickStep::left(9),
        // Take half of it back.
        ClickStep::right(9),
        // Put that half down in the next slot.
        ClickStep::left(10),
        // Move that half back to the hotbar.
        ClickStep::shift_left(10),
    ];

    #[must_use]
    pub fn next_step(&self) -> Option<ClickStep> {
        Self::STEPS.get(self.next).copied()
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        self.next >= Self::STEPS.len()
    }

    pub(crate) fn button(&self) -> ButtonState {
        self.button
    }

    pub(crate) fn press(&mut self, button: ButtonState) {
        self.button = button;
    }

    pub(crate) fn release(&mut self) {
        self.button = ButtonState::Up;
        self.next = self.next.saturating_add(1);
    }
}
