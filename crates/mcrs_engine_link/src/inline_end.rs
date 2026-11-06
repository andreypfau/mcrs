use std::collections::VecDeque;
use std::vec::Drain;

use crate::stage::Stage;
use crate::step::Step;
use crate::tasks::Tasks;

/// A far end on the thread of its caller. The stage runs only when the caller asks.
///
/// A [`Link`](crate::Link) under [`Placement::Inline`](crate::Placement::Inline) holds one. A
/// caller that steps a stage by hand, such as a test that looks into the stage between its
/// steps, holds one of its own.
pub struct InlineEnd<S: Stage> {
    pub(crate) stage: S,
    pub(crate) tasks: Tasks<S::TaskOutput>,
    outputs: Vec<S::Output>,
}

impl<S: Stage> InlineEnd<S> {
    #[must_use]
    pub fn new(stage: S) -> Self {
        Self {
            stage,
            tasks: Tasks::new(),
            outputs: Vec::new(),
        }
    }

    #[must_use]
    pub fn stage(&self) -> &S {
        &self.stage
    }

    pub fn stage_mut(&mut self) -> &mut S {
        &mut self.stage
    }

    pub fn receive(&mut self, input: S::Input) {
        self.stage.receive(input);
    }

    /// Gives the stage the task outcomes that have arrived, steps it once, and hands out what
    /// it emitted. It waits for nothing.
    pub fn step(&mut self) -> Drain<'_, S::Output> {
        while let Some(outcome) = self.tasks.try_take() {
            self.stage.finish(outcome);
        }
        self.stage
            .step(&mut Step::new(&mut self.outputs, &mut self.tasks));
        self.outputs.drain(..)
    }

    /// Blocks until a task of the stage ends, gives the stage its outcome, and tells whether a
    /// task ended. It returns at once when no task is open.
    pub fn wait(&mut self) -> bool {
        let Some(outcome) = self.tasks.wait() else {
            return false;
        };
        self.stage.finish(outcome);
        true
    }

    /// Steps the stage until it owes nothing, or until it cannot make progress, and appends its
    /// outputs to `held`. It blocks on the open tasks of the stage meanwhile.
    pub(crate) fn run(&mut self, held: &mut VecDeque<S::Output>) {
        loop {
            held.extend(self.step());
            if self.stage.is_idle() {
                return;
            }
            if self.stage.has_work() {
                continue;
            }
            if !self.wait() {
                return;
            }
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.stage.is_idle()
    }
}
