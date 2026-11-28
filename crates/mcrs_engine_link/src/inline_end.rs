use std::collections::VecDeque;
use std::vec::Drain;

use crate::stage::Stage;
use crate::step::Step;
use crate::tasks::Tasks;

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

    pub fn step(&mut self) -> Drain<'_, S::Output> {
        while let Some(outcome) = self.tasks.try_take() {
            self.stage.finish(outcome);
        }
        self.stage
            .step(&mut Step::new(&mut self.outputs, &mut self.tasks));
        self.outputs.drain(..)
    }

    pub fn wait(&mut self) -> bool {
        let Some(outcome) = self.tasks.wait() else {
            return false;
        };
        self.stage.finish(outcome);
        true
    }

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
