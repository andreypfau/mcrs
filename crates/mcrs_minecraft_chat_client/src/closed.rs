use crate::history_change::HistoryChange;
use crate::outgoing::Outgoing;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    Escape,
    Sent {
        send: Option<Outgoing>,
        history: HistoryChange,
    },
}
