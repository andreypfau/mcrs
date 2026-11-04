/// The rate the client is asked to answer with, in columns a tick. The ceiling is only a guard
/// against a nonsense answer: a client on the same machine as its server measures itself able to
/// take several hundred a tick, and clamping that back to a round number throttles the load for
/// no reason the client asked for.
const MIN_COLUMNS_PER_TICK: f32 = 0.01;
const MAX_COLUMNS_PER_TICK: f32 = 4096.0;
const START_COLUMNS_PER_TICK: f32 = 9.0;

/// Batches allowed in flight once the client has answered one. Until then a single batch is
/// out at a time, so a client that cannot keep up is never sent a second one to prove it.
const MAX_UNACKNOWLEDGED_BATCHES: u32 = 10;

/// How many columns a client takes, by the rate it answers each batch with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BatchPacing {
    batch_quota: f32,
    desired_columns_per_tick: f32,
    unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
}

impl Default for BatchPacing {
    fn default() -> Self {
        Self {
            batch_quota: 0.0,
            desired_columns_per_tick: START_COLUMNS_PER_TICK,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
}

impl BatchPacing {
    /// Grows the batch quota by what the client asks for each call, up to one batch, while the
    /// client has room for another batch.
    pub(crate) fn refill(&mut self) {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches {
            return;
        }
        let desired = self.desired_columns_per_tick;
        self.batch_quota = (self.batch_quota + desired).min(desired.max(1.0));
    }

    /// The columns the next batch may take, or zero while the client has no room for it.
    #[must_use]
    pub(crate) fn allowed(&self) -> usize {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches || self.batch_quota < 1.0
        {
            return 0;
        }
        self.batch_quota as usize
    }

    pub(crate) fn sent_batch(&mut self, columns: u32) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_add(1);
        self.batch_quota -= columns as f32;
    }

    /// The client has taken a batch and says how many columns a tick it managed while doing so.
    pub(crate) fn acknowledge(&mut self, desired_columns_per_tick: f32) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.desired_columns_per_tick = if desired_columns_per_tick.is_nan() {
            MIN_COLUMNS_PER_TICK
        } else {
            desired_columns_per_tick.clamp(MIN_COLUMNS_PER_TICK, MAX_COLUMNS_PER_TICK)
        };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
    }
}
