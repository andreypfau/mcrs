use bevy_tasks::futures::check_ready;
use bevy_tasks::{IoTaskPool, Task};

/// One batch of file writes that the I/O pool runs.
pub(crate) trait WriteJob: Send + 'static {
    type Output: Send + 'static;

    /// Takes the values that `newer` carries over those of `self`.
    fn merge(&mut self, newer: Self);

    fn run(self) -> Self::Output;
}

/// The jobs on their way to the disk: one run at a time on the I/O pool, and the jobs that come
/// while it runs wait as one merged job. Without a pool the job runs at once.
pub(crate) struct BackgroundWriter<J: WriteJob> {
    pending: Option<J>,
    writing: Option<Task<J::Output>>,
}

impl<J: WriteJob> Default for BackgroundWriter<J> {
    fn default() -> Self {
        Self {
            pending: None,
            writing: None,
        }
    }
}

impl<J: WriteJob> BackgroundWriter<J> {
    pub(crate) fn submit(&mut self, job: J, sink: impl FnMut(J::Output)) {
        match &mut self.pending {
            Some(pending) => pending.merge(job),
            None => self.pending = Some(job),
        }
        self.start(sink);
    }

    /// Collects a finished run and starts the job that waits.
    pub(crate) fn reap(&mut self, mut sink: impl FnMut(J::Output)) {
        if let Some(task) = &mut self.writing
            && let Some(done) = check_ready(task)
        {
            self.writing = None;
            sink(done);
        }
        self.start(sink);
    }

    #[cfg(test)]
    pub(crate) fn flush(&mut self, mut sink: impl FnMut(J::Output)) {
        self.start(&mut sink);
        while let Some(task) = self.writing.take() {
            sink(bevy_tasks::futures_lite::future::block_on(task));
            self.start(&mut sink);
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.pending.is_none() && self.writing.is_none()
    }

    fn start(&mut self, mut sink: impl FnMut(J::Output)) {
        if self.writing.is_some() {
            return;
        }
        let Some(job) = self.pending.take() else {
            return;
        };
        match IoTaskPool::try_get() {
            Some(pool) => self.writing = Some(pool.spawn(async move { job.run() })),
            None => sink(job.run()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct Sum(Vec<u32>, Option<mpsc::Receiver<()>>);

    impl WriteJob for Sum {
        type Output = Vec<u32>;

        fn merge(&mut self, newer: Self) {
            self.0.extend(newer.0);
        }

        fn run(self) -> Vec<u32> {
            if let Some(gate) = self.1 {
                let _ = gate.recv();
            }
            self.0
        }
    }

    #[test]
    fn jobs_that_arrive_during_a_run_wait_and_run_merged_after_it() {
        IoTaskPool::get_or_init(bevy_tasks::TaskPool::new);
        let (open, gate) = mpsc::channel();
        let mut writer = BackgroundWriter::default();
        let mut runs = Vec::new();

        writer.submit(Sum(vec![1], Some(gate)), |out| runs.push(out));
        writer.submit(Sum(vec![2], None), |out| runs.push(out));
        writer.submit(Sum(vec![3], None), |out| runs.push(out));
        writer.reap(|out| runs.push(out));
        assert!(
            runs.is_empty() && !writer.is_idle(),
            "the first run is in flight"
        );

        open.send(()).unwrap();
        writer.flush(|out| runs.push(out));
        assert_eq!(runs, [vec![1], vec![2, 3]]);
        assert!(writer.is_idle());
    }
}
