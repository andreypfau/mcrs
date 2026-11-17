use crate::ControlFlags;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GoalSpec {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interrupt {
    Permitted,
    Refused,
}
