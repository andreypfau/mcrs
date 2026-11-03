use std::future::Future;

use bevy_tasks::{Task, TaskPool};

/// Work handed to a task pool. Each task's output comes back through a channel, which its
/// owner blocks on.
pub(crate) struct Handoff<T> {
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
pub(crate) struct HandedOff {
    pub(crate) id: u64,
    #[expect(dead_code, reason = "held so that dropping the stage cancels its task")]
    pub(crate) task: Task<()>,
}

/// The output of the task with `id`. `None` when the task was dropped before it had one, on
/// cancellation or at shutdown.
pub(crate) struct Finished<T> {
    pub(crate) id: u64,
    pub(crate) output: Option<T>,
}

impl<T: Send + 'static> Handoff<T> {
    pub(crate) fn spawn(
        &mut self,
        pool: &TaskPool,
        work: impl Future<Output = T> + Send + 'static,
    ) -> HandedOff {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut delivery = Delivery {
            id,
            sender: Some(self.sender.clone()),
        };
        let task = pool.spawn(async move {
            let output = work.await;
            delivery.send(Some(output));
        });
        HandedOff { id, task }
    }
}

impl<T> Handoff<T> {
    /// Where the outputs arrive.
    pub(crate) fn receiver(&self) -> &flume::Receiver<Finished<T>> {
        &self.receiver
    }
}

/// Sends a task's output once, and `None` if the task is dropped before it has one.
struct Delivery<T> {
    id: u64,
    sender: Option<flume::Sender<Finished<T>>>,
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

    #[test]
    fn a_task_dropped_before_its_output_hands_back_none() {
        let handoff = Handoff::<u8>::default();
        let delivery = Delivery {
            id: 3,
            sender: Some(handoff.sender.clone()),
        };

        drop(delivery);

        let handed_back: Vec<(u64, Option<u8>)> = handoff
            .receiver()
            .try_iter()
            .map(|finished| (finished.id, finished.output))
            .collect();
        assert_eq!(handed_back, [(3, None)]);
    }
}
