use crate::task_end::{TaskEnd, TaskOutcome};
use crate::tasks::Tasks;

pub trait Stage: Send + Sized + 'static {
    const NAME: &'static str;

    type Input: Send + 'static;
    type Output: Send + 'static;
    type TaskOutput: Send + 'static;

    fn receive(&mut self, input: Self::Input);

    fn finish(&mut self, outcome: TaskOutcome<Self::TaskOutput>) {
        drop(outcome);
    }

    fn step(&mut self, step: &mut Step<'_, Self>);

    fn has_work(&self) -> bool;

    fn is_idle(&self) -> bool {
        !self.has_work()
    }
}

pub struct Step<'a, S: Stage> {
    outputs: &'a mut Vec<S::Output>,
    tasks: &'a mut Tasks<S::TaskOutput>,
}

impl<'a, S: Stage> Step<'a, S> {
    pub(crate) fn new(
        outputs: &'a mut Vec<S::Output>,
        tasks: &'a mut Tasks<S::TaskOutput>,
    ) -> Self {
        Self { outputs, tasks }
    }

    pub fn emit(&mut self, output: S::Output) {
        self.outputs.push(output);
    }

    pub fn begin_task(&mut self) -> TaskEnd<S::TaskOutput> {
        self.tasks.begin()
    }
}
