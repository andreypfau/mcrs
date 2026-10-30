use crate::history_change::HistoryChange;
use crate::outgoing::Outgoing;

/// Why the chat screen closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    Escape,
    /// Enter closed the screen, with what to send, if anything.
    Sent {
        send: Option<Outgoing>,
        history: HistoryChange,
    },
}
