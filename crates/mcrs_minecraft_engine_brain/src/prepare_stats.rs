use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PrepareStats {
    pub(crate) waits: u64,
    pub(crate) prepare: Duration,
    pub(crate) life_cycle: Duration,
    pub(crate) tile_tasks: Duration,
    pub(crate) publish: Duration,
}
