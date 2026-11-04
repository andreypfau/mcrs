use std::collections::VecDeque;

use crate::stage::Stage;
use crate::step::Step;
use crate::tasks::Tasks;

/// A far end that runs on the thread of the near end.
pub(crate) struct InlineEnd<S: Stage> {
    stage: S,
    tasks: Tasks<S::TaskOutput>,
    outputs: Vec<S::Output>,
}

impl<S: Stage> InlineEnd<S> {
    pub(crate) fn new(stage: S) -> Self {
        Self {
            stage,
            tasks: Tasks::new(),
            outputs: Vec::new(),
        }
    }

    pub(crate) fn receive(&mut self, input: S::Input) {
        self.stage.receive(input);
    }

    /// Steps the stage until it owes nothing, or until it cannot make progress, and appends its
    /// outputs to `held`. It blocks on the open tasks of the stage meanwhile.
    pub(crate) fn run(&mut self, held: &mut VecDeque<S::Output>) {
        loop {
            while let Some(outcome) = self.tasks.try_take() {
                self.stage.finish(outcome);
            }
            self.stage
                .step(&mut Step::new(&mut self.outputs, &mut self.tasks));
            held.extend(self.outputs.drain(..));
            if self.stage.is_idle() {
                return;
            }
            if self.stage.has_work() {
                continue;
            }
            let Some(outcome) = self.tasks.wait() else {
                return;
            };
            self.stage.finish(outcome);
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.stage.is_idle()
    }
}
