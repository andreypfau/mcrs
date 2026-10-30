/// Whether a send changed the command history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryChange {
    Unchanged,
    /// The caller writes the command history again.
    Changed,
}
