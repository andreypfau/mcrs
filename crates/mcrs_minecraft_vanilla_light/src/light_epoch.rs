use bevy_tasks::{Task, TaskPool};

use mcrs_engine_link::{TaskEnd, TaskId};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_light_engine::epoch::{LightJob, LightUpdate};

/// The epochs in flight, each with the area it may still write into.
#[derive(Default)]
pub(crate) struct LightEpoch {
    in_flight: Vec<InFlight>,
}

struct InFlight {
    id: TaskId,
    area: BoundingBox,
    /// Dropping it cancels the epoch.
    _task: Task<()>,
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
        self.in_flight
            .iter()
            .any(|epoch| epoch.area.intersects(area))
    }

    pub(crate) fn areas(&self) -> Vec<BoundingBox> {
        self.in_flight.iter().map(|epoch| epoch.area).collect()
    }

    /// Runs `job` on `pool` and finishes `end` with its output.
    pub(crate) fn spawn(&mut self, pool: &TaskPool, end: TaskEnd<LightUpdate>, job: LightJob) {
        let id = end.id();
        let area = job.area();
        let task = pool.spawn(async move {
            end.finish(job.run());
        });
        self.in_flight.push(InFlight {
            id,
            area,
            _task: task,
        });
    }

    /// Takes the epoch with `id` off the epochs in flight, and tells whether it was one.
    pub(crate) fn retire(&mut self, id: TaskId) -> bool {
        let Some(at) = self.in_flight.iter().position(|epoch| epoch.id == id) else {
            return false;
        };
        self.in_flight.swap_remove(at);
        true
    }
}
