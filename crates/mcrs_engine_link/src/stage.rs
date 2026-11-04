use crate::step::Step;
use crate::task_end::TaskOutcome;

/// A step of plain Rust that turns input messages into output messages.
///
/// A stage owns all of its state, starts no thread and reads no clock. It may hand work to a
/// pool of its own choice through [`Step::begin_task`], and gets each outcome back through
/// [`Stage::finish`]. The far end of its [`Link`](crate::Link) calls it from one thread at a
/// time.
pub trait Stage: Send + Sized + 'static {
    /// The name of the thread of the stage under [`Placement::Thread`](crate::Placement::Thread).
    const NAME: &'static str;

    type Input: Send + 'static;
    type Output: Send + 'static;
    /// What a task of the stage hands back.
    type TaskOutput: Send + 'static;

    fn receive(&mut self, input: Self::Input);

    fn finish(&mut self, outcome: TaskOutcome<Self::TaskOutput>);

    /// Does the work that the inputs and the task outcomes so far allow, and emits its outputs.
    fn step(&mut self, step: &mut Step<'_, Self>);

    /// Whether a further step makes progress without a new input or a task outcome.
    fn has_work(&self) -> bool;

    /// Whether the stage owes no output for any input it received.
    fn is_idle(&self) -> bool;
}
