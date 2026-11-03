use bevy_tasks::{Task, TaskPool};
use flume::Sender;

use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_light_engine::epoch::{LightJob, LightUpdate};

use crate::light_inbox::{FinishedEpoch, LightInbox};

/// The epochs in flight, each with the area it may still write into.
#[derive(Default)]
pub(crate) struct LightEpoch {
    in_flight: Vec<InFlight>,
    next_id: u64,
}

struct InFlight {
    id: u64,
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

    /// Runs `job` on `pool` and hands its output to `inbox` once it is done.
    pub(crate) fn spawn(&mut self, pool: &TaskPool, inbox: Sender<LightInbox>, job: LightJob) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let area = job.area();
        let mut delivery = Delivery {
            id,
            inbox: Some(inbox),
        };
        let task = pool.spawn(async move {
            let update = job.run();
            delivery.send(Some(update));
        });
        self.in_flight.push(InFlight {
            id,
            area,
            _task: task,
        });
    }

    /// Takes the epoch with `id` off the epochs in flight, and tells whether it was one.
    pub(crate) fn retire(&mut self, id: u64) -> bool {
        let Some(at) = self.in_flight.iter().position(|epoch| epoch.id == id) else {
            return false;
        };
        self.in_flight.swap_remove(at);
        true
    }
}

/// Sends an epoch's output once, and `None` if the epoch is dropped before it has one.
struct Delivery {
    id: u64,
    inbox: Option<Sender<LightInbox>>,
}

impl Delivery {
    fn send(&mut self, update: Option<LightUpdate>) {
        let Some(inbox) = self.inbox.take() else {
            return;
        };
        // Fails only once the actor has gone, and with it the epoch's purpose.
        let _ = inbox.send(LightInbox::Finished(FinishedEpoch {
            id: self.id,
            update,
        }));
    }
}

impl Drop for Delivery {
    fn drop(&mut self) {
        self.send(None);
    }
}
