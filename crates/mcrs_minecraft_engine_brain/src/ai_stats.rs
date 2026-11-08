use std::time::Duration;

/// What the last AI tick did and cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiStats {
    pub mobs: usize,
    /// The wall time of the snapshot, the application and the controls of the hot side.
    pub prepare: Duration,
    /// The parts of `prepare`: the life cycle, the parallel tile tasks and the publication.
    pub life_cycle: Duration,
    pub tile_tasks: Duration,
    pub publish: Duration,
    /// The wall time of the stage tick.
    pub stage: Duration,
    pub evaluations: u64,
    pub decisions: usize,
    pub applied: usize,
    /// The ticks in which a due decision was not ready.
    pub waits: u64,
}
