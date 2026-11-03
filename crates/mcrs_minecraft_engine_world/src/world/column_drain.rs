use std::future::Future;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::{ScheduleLabel, SystemSet};
use bevy_tasks::{Task, TaskPool};

/// Puts the columns whose sections landed on the wire, at the end of every tick and between
/// ticks, so a column read from the save does not wait for the next tick.
///
/// Between ticks a dimension drains only when it asks to through its [`DrainWake`]: what the
/// drain acts on changes only when work the dimension handed off finishes.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnDrain;

/// The stages of a [`ColumnDrain`], in run order. Each acts only on what is pending.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnDrainSet {
    /// Finished columns come off the pool into their sections.
    Collect,
    /// The column index, block entities and heightmaps catch up with what landed.
    Reconcile,
    /// Sections spawn, and the columns they need are requested.
    Request,
    /// A finished light epoch is retired and the next one dispatched.
    Light,
    /// Ready columns go out and the outbox is flushed towards the host.
    Send,
}

/// Where the server loop waits for the dimensions to ask for a drain between ticks.
#[derive(Resource)]
pub struct DrainWakes {
    sender: flume::Sender<Entity>,
    receiver: flume::Receiver<Entity>,
}

impl Default for DrainWakes {
    fn default() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }
}

impl DrainWakes {
    /// The wake of the dimension whose sub-app is labelled with `dim`.
    pub fn for_dimension(&self, dim: Entity) -> DrainWake {
        DrainWake {
            dim,
            sender: self.sender.clone(),
        }
    }

    /// The labels of the dimensions that asked for a drain, as they ask.
    pub fn receiver(&self) -> &flume::Receiver<Entity> {
        &self.receiver
    }
}

/// Asks the server loop to drain this dimension before the next tick.
#[derive(Resource, Clone)]
pub struct DrainWake {
    dim: Entity,
    sender: flume::Sender<Entity>,
}

impl DrainWake {
    pub fn wake(&self) {
        // Fails only once the host has gone, and then nobody drains anything.
        let _ = self.sender.send(self.dim);
    }
}

/// Work handed to a task pool. Each task's output comes back through a channel, and the task
/// wakes the drain only once the output is in it, so the drain never polls a task that is about
/// to finish.
pub struct Handoff<T> {
    sender: flume::Sender<Finished<T>>,
    receiver: flume::Receiver<Finished<T>>,
    next_id: u64,
}

impl<T> Default for Handoff<T> {
    fn default() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            sender,
            receiver,
            next_id: 0,
        }
    }
}

/// A task of a [`Handoff`]. Dropping it cancels the task.
pub struct HandedOff {
    pub id: u64,
    pub task: Task<()>,
}

/// The output of the task with `id`. `None` when the task ended without one: it panicked, or
/// it was cancelled.
pub struct Finished<T> {
    pub id: u64,
    pub output: Option<T>,
}

impl<T: Send + 'static> Handoff<T> {
    pub fn spawn(
        &mut self,
        pool: &TaskPool,
        wake: Option<&DrainWake>,
        work: impl Future<Output = T> + Send + 'static,
    ) -> HandedOff {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut delivery = Delivery {
            id,
            sender: Some(self.sender.clone()),
            wake: wake.cloned(),
        };
        let task = pool.spawn(async move {
            let output = work.await;
            delivery.send(Some(output));
        });
        HandedOff { id, task }
    }
}

impl<T> Handoff<T> {
    pub fn finished(&self) -> flume::TryIter<'_, Finished<T>> {
        self.receiver.try_iter()
    }
}

/// Sends a task's output once, and `None` if the task is dropped before it has one.
struct Delivery<T> {
    id: u64,
    sender: Option<flume::Sender<Finished<T>>>,
    wake: Option<DrainWake>,
}

impl<T> Delivery<T> {
    fn send(&mut self, output: Option<T>) {
        let Some(sender) = self.sender.take() else {
            return;
        };
        // Fails only once the owner of the handoff has gone, and with it the tasks' purpose.
        let _ = sender.send(Finished {
            id: self.id,
            output,
        });
        if let Some(wake) = &self.wake {
            wake.wake();
        }
    }
}

impl<T> Drop for Delivery<T> {
    fn drop(&mut self) {
        self.send(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const PATIENCE: Duration = Duration::from_secs(5);

    fn handed_back<T>(handoff: &Handoff<T>) -> Vec<(u64, Option<T>)> {
        handoff
            .finished()
            .map(|finished| (finished.id, finished.output))
            .collect()
    }

    #[test]
    fn a_task_wakes_the_drain_once_its_output_is_in() {
        let wakes = DrainWakes::default();
        let pool = TaskPool::new();
        let mut handoff = Handoff::default();

        let task = handoff.spawn(
            &pool,
            Some(&wakes.for_dimension(Entity::PLACEHOLDER)),
            async { 7 },
        );

        assert_eq!(
            wakes.receiver().recv_timeout(PATIENCE),
            Ok(Entity::PLACEHOLDER)
        );
        assert_eq!(handed_back(&handoff), [(task.id, Some(7))]);
    }

    #[test]
    fn a_task_dropped_before_its_output_hands_back_none() {
        let wakes = DrainWakes::default();
        let handoff = Handoff::<u8>::default();
        let delivery = Delivery {
            id: 3,
            sender: Some(handoff.sender.clone()),
            wake: Some(wakes.for_dimension(Entity::PLACEHOLDER)),
        };

        drop(delivery);

        assert_eq!(wakes.receiver().try_recv(), Ok(Entity::PLACEHOLDER));
        assert_eq!(handed_back(&handoff), [(3, None)]);
    }
}
