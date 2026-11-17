#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum InstanceChange {
    Changed,
    Unchanged,
}
