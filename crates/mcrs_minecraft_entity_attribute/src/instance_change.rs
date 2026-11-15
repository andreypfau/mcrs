/// Whether a call changed an attribute instance (`attributes.md`, ATR-INS-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum InstanceChange {
    Changed,
    Unchanged,
}
