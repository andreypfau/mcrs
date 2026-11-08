use crate::MobId;

/// An event of the hot tick that a decision of a later tick must see. A fact wakes the brain of its
/// mob at its next full evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fact {
    pub mob: MobId,
    pub kind: FactKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FactKind {
    /// The damage record changed.
    Hurt,
    /// The path of the last applied path intent ended: done, stuck, timed out or cleared.
    PathEnded,
    /// The look-around of the last applied look intent ended.
    LookEnded,
    /// A passenger took or lost the control of the mob, or the disabled flags changed.
    ControlChanged,
    /// The host dropped a stale decision of the mob.
    Dropped,
}
