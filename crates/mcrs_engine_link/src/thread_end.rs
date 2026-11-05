use flume::{Receiver, RecvError, Selector, SendError, Sender, TryRecvError};

use crate::inline_end::InlineEnd;
use crate::reply::Reply;
use crate::stage::Stage;
use crate::task_end::TaskOutcome;

/// A far end on a thread of its own. It blocks until an input or a task outcome arrives, takes
/// in everything that waits, steps the stage and sends the outputs.
///
/// It stops once the near end is gone: the inputs disconnect, or a reply finds nobody.
pub(crate) struct ThreadEnd<S: Stage> {
    end: InlineEnd<S>,
    inputs: Receiver<S::Input>,
    replies: Sender<Reply<S::Output>>,
    wake: Option<Sender<()>>,
    seen: u64,
    reported: u64,
}

enum Event<I, T> {
    Input(Result<I, RecvError>),
    Outcome(Result<TaskOutcome<T>, RecvError>),
}

impl<S: Stage> ThreadEnd<S> {
    pub(crate) fn new(
        stage: S,
        inputs: Receiver<S::Input>,
        replies: Sender<Reply<S::Output>>,
        wake: Option<Sender<()>>,
    ) -> Self {
        Self {
            end: InlineEnd::new(stage),
            inputs,
            replies,
            wake,
            seen: 0,
            reported: 0,
        }
    }

    pub(crate) fn run(mut self) {
        loop {
            if !self.end.stage.has_work() {
                let event = Selector::new()
                    .recv(&self.inputs, Event::Input)
                    .recv(self.end.tasks.receiver(), Event::Outcome)
                    .wait();
                match event {
                    Event::Input(Ok(input)) => self.receive(input),
                    Event::Outcome(Ok(outcome)) => {
                        self.end.tasks.taken();
                        self.end.stage.finish(outcome);
                    }
                    Event::Input(Err(_)) | Event::Outcome(Err(_)) => return,
                }
            }
            loop {
                match self.inputs.try_recv() {
                    Ok(input) => self.receive(input),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if self.step().is_err() {
                return;
            }
        }
    }

    fn receive(&mut self, input: S::Input) {
        self.seen = self.seen.wrapping_add(1);
        self.end.receive(input);
    }

    fn step(&mut self) -> Result<(), SendError<Reply<S::Output>>> {
        let mut emitted = false;
        for output in self.end.step() {
            emitted = true;
            self.replies.send(Reply::Output(output))?;
        }
        if emitted && let Some(wake) = &self.wake {
            // A full channel already holds a wake, and a closed one has nobody to wake.
            let _ = wake.try_send(());
        }
        // After the outputs, so the near end holds them by the time it counts the inputs answered.
        if self.seen != self.reported && self.end.is_idle() {
            self.replies.send(Reply::Answered { seen: self.seen })?;
            self.reported = self.seen;
        }
        Ok(())
    }
}
