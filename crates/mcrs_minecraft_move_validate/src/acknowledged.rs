use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Acknowledged {
    pub tick: u64,
    pub sent_at: Duration,
}
