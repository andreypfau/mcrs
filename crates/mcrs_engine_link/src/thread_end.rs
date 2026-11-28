use flume::{Receiver, SendError, Sender, TryRecvError};

use crate::inline_end::InlineEnd;
use crate::reply::Reply;
use crate::stage::Stage;

pub(crate) struct ThreadEnd<S: Stage> {
    end: InlineEnd<S>,
    inputs: Receiver<S::Input>,
    replies: Sender<Reply<S::Output>>,
    wake: Option<Sender<()>>,
    seen: u64,
    reported: u64,
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
            if !self.end.stage().has_work()
                && let Some(input) = self.end.wait_for(&self.inputs)
            {
                self.receive(input);
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
            let _ = wake.try_send(());
        }
        if self.seen != self.reported && self.end.is_idle() {
            self.replies.send(Reply::Answered { seen: self.seen })?;
            self.reported = self.seen;
        }
        Ok(())
    }
}
