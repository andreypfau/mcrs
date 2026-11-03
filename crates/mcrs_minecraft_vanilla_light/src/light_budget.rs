use bevy_ecs::resource::Resource;
use bevy_tasks::{AsyncComputeTaskPool, available_parallelism};

/// How much lighting work may be under way at once.
///
/// The point is not to finish sooner but to keep any single pass bounded: light
/// is allowed to lag, a pass that never ends is not.
#[derive(Resource, Copy, Clone, Debug, PartialEq, Eq)]
pub struct LightBudget {
    /// Cells of working field one epoch may cover. Counting influences instead
    /// bounds nothing: the batch is unioned into a single field and the cost of
    /// an epoch is that field's volume, whether two influences or two thousand
    /// spanned it.
    pub cells_per_epoch: u64,
    /// Epochs allowed in flight together.
    pub epochs_in_flight: usize,
}

impl Default for LightBudget {
    fn default() -> Self {
        Self {
            // The influence radius and the section rounding are paid for on the
            // perimeter, and a perimeter grows with the square root of what it
            // encloses, so a batch is worth making large — but only now that the
            // queue hands out a block of columns rather than a slice of the ring
            // it orders them in. A few dozen columns of a full-height world.
            cells_per_epoch: 8 << 20,
            // A slot beyond what the pool can actually start holds no epoch but
            // still blocks its area against the next batch, so the ceiling is
            // the pool's width and not the machine's.
            epochs_in_flight: AsyncComputeTaskPool::try_get()
                .map(|pool| pool.thread_num())
                .unwrap_or_else(available_parallelism),
        }
    }
}
