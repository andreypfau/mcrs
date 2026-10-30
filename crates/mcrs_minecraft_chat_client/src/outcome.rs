use crate::closed::Closed;

/// What the chat screen did with an input event.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The screen did nothing; the caller may give the event to someone else.
    Ignored,
    /// The screen used the event.
    Handled,
    /// The screen closed.
    Closed(Closed),
    /// Page Up (`1`) or Page Down (`-1`) scrolls the chat log by `perPage − 1` lines.
    Page(isize),
    /// The caller puts this text on the clipboard.
    Copy(String),
}
