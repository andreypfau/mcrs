use bevy_ecs::resource::Resource;

/// How many section columns the light stage takes in before it hands out the next epochs.
///
/// Loading a column costs a 256 block column sky rescan. Only section loads and
/// unloads are held back — a block change takes effect the round it arrives,
/// and there are never thousands of those at once.
#[derive(Resource, Copy, Clone, Debug, PartialEq, Eq)]
pub struct IntakeBudget {
    pub columns_per_round: usize,
}

impl Default for IntakeBudget {
    fn default() -> Self {
        // The rescan is cheap next to the epochs it feeds, and holding it back
        // only starves them: a bulk load at render distance 96 admits its whole
        // 38025 columns in under a second of intake and spends the rest of the
        // load relaxing. The bound only keeps the first epochs from waiting
        // behind the whole intake.
        Self {
            columns_per_round: 1024,
        }
    }
}
