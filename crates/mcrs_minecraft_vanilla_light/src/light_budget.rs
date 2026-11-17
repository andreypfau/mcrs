use bevy_ecs::resource::Resource;
use bevy_tasks::{AsyncComputeTaskPool, available_parallelism};

#[derive(Resource, Copy, Clone, Debug, PartialEq, Eq)]
pub struct LightBudget {
    pub cells_per_epoch: u64,
    pub epochs_in_flight: usize,
}

impl Default for LightBudget {
    fn default() -> Self {
        Self {
            cells_per_epoch: 8 << 20,
            epochs_in_flight: AsyncComputeTaskPool::try_get()
                .map(|pool| pool.thread_num())
                .unwrap_or_else(available_parallelism),
        }
    }
}
