use crate::MobId;

/// An event of the hot tick that a decision of a later tick must see: the wake events of
/// GOAL-WAKE-005. A fact wakes the brain of its mob at its first full evaluation whose snapshot
/// holds it.
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
    /// The path of the last applied path intent became done, or the mob lost it: the follower
    /// stopped it, the request failed or a reflex cleared it.
    PathEnded,
    /// A passenger mounted the mob or left it, or the controller of the mob changed.
    RiderChanged,
    /// The disabled flags of the mob changed.
    DisabledChanged,
    /// The idle time reset while it was 100 or more.
    IdleReset,
    /// The mob entered love, or became a baby.
    LoveOrBaby,
    /// The mob did an AI step after a tick with no AI step.
    Resumed,
    /// The host dropped a stale decision of the mob.
    Dropped,
}
