use bevy_tasks::{Task, TaskPool};

use mcrs_engine_link::{InFlight, TaskEnd, TaskId};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_light_engine::epoch::{LightJob, LightUpdate};

/// The epochs in flight, each with the area it may still write into.
#[derive(Default)]
pub(crate) struct LightEpoch {
    in_flight: InFlight<BoundingBox, Task<()>>,
}

impl LightEpoch {
    pub(crate) fn is_running(&self) -> bool {
        !self.in_flight.is_empty()
    }

    pub(crate) fn running_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Whether an epoch under way may still write into `area`.
    pub(crate) fn touches(&self, area: BoundingBox) -> bool {
        self.in_flight.iter().any(|epoch| epoch.intersects(area))
    }

    pub(crate) fn areas(&self) -> Vec<BoundingBox> {
        self.in_flight.iter().copied().collect()
    }

    /// Runs `job` on `pool` and finishes `end` with its output.
    pub(crate) fn spawn(&mut self, pool: &TaskPool, end: TaskEnd<LightUpdate>, job: LightJob) {
        let id = end.id();
        let area = job.area();
        let task = pool.spawn(async move {
            end.finish(job.run());
        });
        self.in_flight.push(id, area, task);
    }

    /// Takes the epoch with `id` off the epochs in flight, and tells whether it was one.
    pub(crate) fn retire(&mut self, id: TaskId) -> bool {
        self.in_flight.retire(id).is_some()
    }
}
