#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeState {
    NotApplied,
    Window,
    Applied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeView {
    pub change: u64,
    pub state: ChangeState,
}
