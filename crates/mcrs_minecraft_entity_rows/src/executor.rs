pub trait Executor {
    fn threads(&self) -> usize;

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
