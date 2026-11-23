use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeState {
    NotApplied,
    Window,
    Applied,
}

impl ChangeState {
    #[must_use]
    pub fn of_change_sent_in(sent_tick: u64, acknowledged_tick: u64) -> Self {
        match acknowledged_tick.cmp(&sent_tick) {
            Ordering::Greater => Self::Applied,
            Ordering::Equal => Self::Window,
            Ordering::Less => Self::NotApplied,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeView {
    pub change: u64,
    pub state: ChangeState,
}
