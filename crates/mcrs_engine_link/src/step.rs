use crate::stage::Stage;
use crate::task_end::TaskEnd;
use crate::tasks::Tasks;

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
