use flume::{Receiver, Sender};

use crate::task_end::{TaskEnd, TaskId, TaskOutcome};

/// The tasks a stage opened, and the way their outcomes come back.
pub(crate) struct Tasks<T> {
    sender: Sender<TaskOutcome<T>>,
    receiver: Receiver<TaskOutcome<T>>,
    next_id: u64,
    open: usize,
}

impl<T> Tasks<T> {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            sender,
            receiver,
            next_id: 0,
            open: 0,
        }
    }

    pub(crate) fn begin(&mut self) -> TaskEnd<T> {
        let id = TaskId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.open = self.open.saturating_add(1);
        TaskEnd::new(id, self.sender.clone())
    }

    pub(crate) fn receiver(&self) -> &Receiver<TaskOutcome<T>> {
        &self.receiver
    }

    pub(crate) fn try_take(&mut self) -> Option<TaskOutcome<T>> {
        let outcome = self.receiver.try_recv().ok()?;
        self.taken();
        Some(outcome)
    }

    /// Blocks for the next outcome, or gives `None` at once when no task is open.
    pub(crate) fn wait(&mut self) -> Option<TaskOutcome<T>> {
        if self.open == 0 {
            return None;
        }
        // Never disconnects, because `self` holds a sender.
        let outcome = self.receiver.recv().ok()?;
        self.taken();
        Some(outcome)
    }

    /// Counts an outcome taken off the receiver by other means than `self`.
    pub(crate) fn taken(&mut self) {
        self.open = self.open.saturating_sub(1);
    }
}
