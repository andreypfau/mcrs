use flume::{Receiver, RecvError, Selector, Sender};

use crate::task_end::{TaskEnd, TaskId, TaskOutcome};
use crate::waited::Waited;

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

    pub(crate) fn select<I>(&mut self, inputs: &Receiver<I>) -> Waited<I, TaskOutcome<T>> {
        enum Event<I, T> {
            Input(Result<I, RecvError>),
            Outcome(Result<TaskOutcome<T>, RecvError>),
        }
        let event = Selector::new()
            .recv(inputs, Event::Input)
            .recv(&self.receiver, Event::Outcome)
            .wait();
        match event {
            Event::Input(Ok(input)) => Waited::Input(input),
            Event::Outcome(Ok(outcome)) => {
                self.taken();
                Waited::Outcome(outcome)
            }
            Event::Input(Err(_)) | Event::Outcome(Err(_)) => Waited::Closed,
        }
    }

    pub(crate) fn try_take(&mut self) -> Option<TaskOutcome<T>> {
        let outcome = self.receiver.try_recv().ok()?;
        self.taken();
        Some(outcome)
    }

    pub(crate) fn wait(&mut self) -> Option<TaskOutcome<T>> {
        if self.open == 0 {
            return None;
        }
        let outcome = self.receiver.recv().ok()?;
        self.taken();
        Some(outcome)
    }

    fn taken(&mut self) {
        self.open = self.open.saturating_sub(1);
    }
}
