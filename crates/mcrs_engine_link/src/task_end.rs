use flume::Sender;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub(crate) u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskOutcome<T> {
    Finished { id: TaskId, output: T },
    Dropped { id: TaskId },
}

impl<T> TaskOutcome<T> {
    #[must_use]
    pub fn id(&self) -> TaskId {
        match self {
            Self::Finished { id, .. } | Self::Dropped { id } => *id,
        }
    }
}

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
        let _ = outcomes.send(outcome);
    }
}

impl<T> Drop for TaskEnd<T> {
    fn drop(&mut self) {
        let id = self.id;
        self.send(TaskOutcome::Dropped { id });
    }
}
