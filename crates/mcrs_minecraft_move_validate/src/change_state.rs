/// Whether the client can have applied a predicted-state change by a client tick. The order
/// is the order of progress: a change never goes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeState {
    /// The probe before the change has no answer for the tick.
    NotApplied,
    /// The probe before has an answer and the probe after has none: the client can have the
    /// change or not.
    Window,
    /// The probe after the change has an answer for the tick.
    Applied,
}

/// One change, numbered from 0 in the order of the writes, and its state for the last client
/// tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangeView {
    pub change: u64,
    pub state: ChangeState,
}
