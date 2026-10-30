use crate::closed::Closed;

/// What Escape does with an unsent input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnsentDraft {
    #[default]
    Discard,
    Save,
}

/// The input the client keeps after the screen closed, if any.
#[must_use]
pub fn keeps_draft(input: &str, closed: Option<&Closed>, unsent: UnsentDraft) -> bool {
    if input.trim().is_empty() {
        return false;
    }
    match closed {
        Some(Closed::Sent { .. }) => false,
        Some(Closed::Escape) => unsent == UnsentDraft::Save,
        None => true,
    }
}
