use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageStats {
    pub decisions: usize,
    pub evaluations: u64,
    pub elapsed: Duration,
}
