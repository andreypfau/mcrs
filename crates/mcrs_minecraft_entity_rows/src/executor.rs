/// Runs the tasks of one parallel step of the rows. The result of a step does not depend on the
/// thread count of the executor or on the order in which its tasks run.
pub trait Executor {
    /// The count of threads that run the tasks; the pass deals four tasks to each.
    fn threads(&self) -> usize;

    /// Runs `work` on each of `tasks` and gives the results in the order of `tasks`.
    fn run<T, R, F>(&self, tasks: Vec<T>, work: &F) -> Vec<R>
    where
        T: Send,
        R: Send + 'static,
        F: Fn(T) -> R + Sync;
}

#[cfg(feature = "bevy")]
impl Executor for bevy_tasks::TaskPool {
    fn threads(&self) -> usize {
        self.thread_num()
    }

    fn run<T, R, F>(&self, tasks: Vec<T>, work: &F) -> Vec<R>
    where
        T: Send,
        R: Send + 'static,
        F: Fn(T) -> R + Sync,
    {
        self.scope(|scope| {
            for task in tasks {
                scope.spawn(async move { work(task) });
            }
        })
    }
}
