use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiStats {
    pub mobs: usize,
    pub prepare: Duration,
    pub life_cycle: Duration,
    pub tile_tasks: Duration,
    pub publish: Duration,
    pub stage: Duration,
    pub evaluations: u64,
    pub decisions: usize,
    pub applied: usize,
    pub waits: u64,
}
