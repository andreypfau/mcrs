use flume::Sender;

/// The identity of a task of a stage, unique for the life of its link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub(crate) u64);

/// How a task of a stage ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskOutcome<T> {
    Finished {
        id: TaskId,
        output: T,
    },
    /// The [`TaskEnd`] was dropped before it was finished: the task was cancelled or panicked,
    /// or its pool shut down.
    Dropped {
        id: TaskId,
    },
}

impl<T> TaskOutcome<T> {
    #[must_use]
    pub fn id(&self) -> TaskId {
        match self {
            Self::Finished { id, .. } | Self::Dropped { id } => *id,
        }
    }
}

/// The end of a task of a stage. Move it into the task, and finish it with the output.
///
/// The stage gets exactly one [`TaskOutcome`] for each end, also when the end is dropped.
pub struct TaskEnd<T> {
    id: TaskId,
    outcomes: Option<Sender<TaskOutcome<T>>>,
}

impl<T> TaskEnd<T> {
    pub(crate) fn new(id: TaskId, outcomes: Sender<TaskOutcome<T>>) -> Self {
        Self {
            id,
            outcomes: Some(outcomes),
        }
    }

    #[must_use]
    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn finish(mut self, output: T) {
        let id = self.id;
        self.send(TaskOutcome::Finished { id, output });
    }

    fn send(&mut self, outcome: TaskOutcome<T>) {
        let Some(outcomes) = self.outcomes.take() else {
            return;
        };
        // Fails only once the stage has gone, and with it the purpose of the task.
        let _ = outcomes.send(outcome);
    }
}

impl<T> Drop for TaskEnd<T> {
    fn drop(&mut self) {
        let id = self.id;
        self.send(TaskOutcome::Dropped { id });
    }
}
