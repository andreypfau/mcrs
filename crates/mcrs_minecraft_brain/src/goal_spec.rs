use crate::ControlFlags;

/// The fixed data of one goal of a goal list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GoalSpec {
    /// A smaller number is a higher priority.
    pub priority: u8,
    pub flags: ControlFlags,
    pub interrupt: Interrupt,
}

impl GoalSpec {
    #[must_use]
    pub const fn new(priority: u8, flags: ControlFlags) -> Self {
        Self {
            priority,
            flags,
            interrupt: Interrupt::Permitted,
        }
    }
}

/// Whether a goal with a smaller priority number can stop a goal while it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interrupt {
    Permitted,
    Refused,
}
