use std::collections::VecDeque;
use std::collections::vec_deque::Drain;
use std::thread;

use flume::{Receiver, Sender};

use crate::inline_end::InlineEnd;
use crate::placement::Placement;
use crate::reply::Reply;
use crate::stage::Stage;
use crate::thread_end::ThreadEnd;

/// The near end of the connection to a [`Stage`]: the inputs go in, the outputs come out.
///
/// The near end sends inputs during its tick and takes the outputs at a point of its choice,
/// such as the start of the next tick. Dropping the link stops the far end.
pub struct Link<S: Stage> {
    far: FarEnd<S>,
    /// Outputs taken off the far end and not handed out yet, oldest first.
    held: VecDeque<S::Output>,
    sent: u64,
    answered: u64,
}

enum FarEnd<S: Stage> {
    Inline(InlineEnd<S>),
    Thread(ThreadLink<S>),
}

struct ThreadLink<S: Stage> {
    inputs: Sender<S::Input>,
    replies: Receiver<Reply<S::Output>>,
}

impl<S: Stage> Link<S> {
    /// Places `stage` as `placement` asks. A stage whose thread does not start answers nothing,
    /// as a stopped far end.
    pub fn new(stage: S, placement: Placement) -> Self {
        Self::place(stage, placement, None)
    }

    /// As [`Self::new`], and a far end on a thread of its own sends `wake` a message each time
    /// it hands out outputs, so that the near end can block on a channel of its choice until
    /// they arrive. An inline far end runs only when the near end asks, so it never wakes.
    pub fn new_with_wake(stage: S, placement: Placement, wake: Sender<()>) -> Self {
        Self::place(stage, placement, Some(wake))
    }

    fn place(stage: S, placement: Placement, wake: Option<Sender<()>>) -> Self {
        let far = match placement {
            Placement::Inline => FarEnd::Inline(InlineEnd::new(stage)),
            Placement::Thread => spawn(stage, wake),
        };
        Self {
            far,
            held: VecDeque::new(),
            sent: 0,
            answered: 0,
        }
    }

    pub fn send(&mut self, input: S::Input) {
        match &mut self.far {
            FarEnd::Inline(end) => end.receive(input),
            FarEnd::Thread(link) => {
                // Fails only once the far end has stopped, and then no answer comes back to wait for.
                if link.inputs.send(input).is_err() {
                    return;
                }
            }
        }
        self.sent = self.sent.wrapping_add(1);
    }

    /// Takes in what the far end has sent so far, so that [`Self::is_answered`] counts it,
    /// and holds the outputs for [`Self::outputs`]. An inline far end runs here.
    fn take_in(&mut self) {
        match &mut self.far {
            FarEnd::Inline(end) => {
                if self.answered == self.sent {
                    return;
                }
                end.run(&mut self.held);
                if end.is_idle() {
                    self.answered = self.sent;
                }
            }
            FarEnd::Thread(link) => {
                for reply in link.replies.try_iter() {
                    hold(&mut self.held, &mut self.answered, reply);
                }
            }
        }
    }

    /// Waits until the far end owes no output for any input sent, and holds the outputs that
    /// arrive meanwhile for [`Self::outputs`]. An inline far end runs here until its stage is
    /// idle or cannot make progress. It resolves at once when the far end has stopped.
    pub async fn answered(&mut self) {
        loop {
            self.take_in();
            if self.is_answered() {
                return;
            }
            let FarEnd::Thread(link) = &self.far else {
                return;
            };
            let Ok(reply) = link.replies.recv_async().await else {
                return;
            };
            hold(&mut self.held, &mut self.answered, reply);
        }
    }

    /// Takes in what the far end has sent, and hands out every output not handed out yet, in
    /// the order the stage emitted them.
    pub fn outputs(&mut self) -> Drain<'_, S::Output> {
        self.take_in();
        self.held.drain(..)
    }

    /// Whether the far end owes no output for any input sent, as far as the outputs taken in
    /// tell.
    #[must_use]
    pub fn is_answered(&self) -> bool {
        self.answered == self.sent
    }

    /// The stage, when it runs [`Placement::Inline`]. A stage on a thread of its own is out of
    /// reach of the near end.
    #[must_use]
    pub fn inline_stage(&self) -> Option<&S> {
        match &self.far {
            FarEnd::Inline(end) => Some(end.stage()),
            FarEnd::Thread(_) => None,
        }
    }
}

fn hold<O>(held: &mut VecDeque<O>, answered: &mut u64, reply: Reply<O>) {
    match reply {
        Reply::Output(output) => held.push_back(output),
        Reply::Answered { seen } => *answered = seen,
    }
}

fn spawn<S: Stage>(stage: S, wake: Option<Sender<()>>) -> FarEnd<S> {
    let (inputs, inputs_receiver) = flume::unbounded();
    let (replies_sender, replies) = flume::unbounded();
    let end = ThreadEnd::new(stage, inputs_receiver, replies_sender, wake);
    let spawned = thread::Builder::new()
        .name(S::NAME.to_owned())
        .spawn(move || end.run());
    if let Err(error) = spawned {
        // The channels of the stage are gone with it, so the link acts as a stopped far end.
        tracing::error!(%error, stage = S::NAME, "the stage thread did not start");
    }
    FarEnd::Thread(ThreadLink { inputs, replies })
}

#[cfg(test)]
mod tests {
    use super::*;

    use bevy_tasks::block_on;

    use crate::step::Step;
    use crate::task_end::TaskOutcome;

    /// Doubles each even input on a thread of a task, and drops the task of each odd input
    /// unfinished.
    #[derive(Default)]
    struct Doubler {
        inputs: Vec<u8>,
        ended: Vec<Option<u8>>,
        open: usize,
    }

    impl Stage for Doubler {
        const NAME: &'static str = "doubler";
        type Input = u8;
        type Output = Option<u8>;
        type TaskOutput = u8;

        fn receive(&mut self, input: u8) {
            self.inputs.push(input);
        }

        fn finish(&mut self, outcome: TaskOutcome<u8>) {
            self.open -= 1;
            self.ended.push(match outcome {
                TaskOutcome::Finished { output, .. } => Some(output),
                TaskOutcome::Dropped { .. } => None,
            });
        }

        fn step(&mut self, step: &mut Step<'_, Self>) {
            for input in self.inputs.drain(..) {
                let end = step.begin_task();
                self.open += 1;
                if input % 2 == 0 {
                    thread::spawn(move || end.finish(input * 2));
                }
            }
            for ended in self.ended.drain(..) {
                step.emit(ended);
            }
        }

        fn has_work(&self) -> bool {
            !self.inputs.is_empty()
        }

        fn is_idle(&self) -> bool {
            self.open == 0 && self.inputs.is_empty() && self.ended.is_empty()
        }
    }

    fn answers(placement: Placement) -> Vec<Option<u8>> {
        let mut link = Link::new(Doubler::default(), placement);
        for input in 1..=4 {
            link.send(input);
        }
        block_on(link.answered());
        assert!(
            link.is_answered(),
            "the far end resolved its wait unanswered"
        );
        let mut outputs: Vec<Option<u8>> = link.outputs().collect();
        outputs.sort_unstable();
        outputs
    }

    #[test]
    fn every_input_is_answered_with_the_outcome_of_its_task() {
        let expected = [None, None, Some(4), Some(8)];
        assert_eq!(answers(Placement::Inline), expected);
        assert_eq!(answers(Placement::Thread), expected);
    }

    #[test]
    fn a_thread_far_end_wakes_the_near_end_once_its_outputs_wait() {
        let (wake, woken) = flume::bounded(1);
        let mut link = Link::new_with_wake(Doubler::default(), Placement::Thread, wake);
        link.send(2);

        woken.recv().expect("the thread wakes the near end");

        let outputs: Vec<Option<u8>> = link.outputs().collect();
        assert_eq!(outputs, [Some(4)]);
    }

    #[test]
    fn the_thread_stops_once_the_link_is_dropped() {
        let link = Link::new(Doubler::default(), Placement::Thread);
        let FarEnd::Thread(ThreadLink { replies, .. }) = &link.far else {
            panic!("the thread started");
        };
        let replies = replies.clone();

        drop(link);

        // The thread holds the only sender of the replies, so the channel disconnects once the
        // thread has ended.
        assert!(
            replies.recv().is_err(),
            "the thread replied with nothing sent to it"
        );
    }
}
