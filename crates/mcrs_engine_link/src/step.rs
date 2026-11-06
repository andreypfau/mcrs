use crate::stage::Stage;
use crate::task_end::TaskEnd;
use crate::tasks::Tasks;

/// What one [`Stage::step`] can do besides changing the state of its stage.
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

    /// Sends `output` to the near end, which takes the outputs in the order of emission.
    pub fn emit(&mut self, output: S::Output) {
        self.outputs.push(output);
    }

    /// Opens a task. The stage gets its outcome through [`Stage::finish`] once the returned end
    /// is finished or dropped.
    pub fn begin_task(&mut self) -> TaskEnd<S::TaskOutput> {
        self.tasks.begin()
    }
}
