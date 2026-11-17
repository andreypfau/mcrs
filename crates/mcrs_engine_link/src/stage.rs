use crate::step::Step;
use crate::task_end::TaskOutcome;

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
