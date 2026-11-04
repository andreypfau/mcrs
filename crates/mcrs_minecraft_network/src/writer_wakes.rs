use std::mem;
use std::sync::Arc;

use tokio::runtime::Handle;

use crate::writer_signal::WriterSignal;

/// The writers that a flush handed batches to, woken together when the flush ends.
///
/// A wake from a thread outside the runtime puts the writer task in the shared queue of the
/// runtime, behind a lock that the idle workers also take. Hundreds of such wakes in a row spend
/// most of their time on that lock. One task on a worker wakes them through the queue of its
/// own worker instead, so the thread of the flush pays for one wake.
pub struct WriterWakes {
    signals: Vec<Arc<WriterSignal>>,
    runtime: Option<Handle>,
}

impl WriterWakes {
    /// Wakes the writers from a task on `runtime`, the runtime of the writers.
    #[must_use]
    pub fn on_runtime(runtime: Handle) -> Self {
        Self {
            signals: Vec::new(),
            runtime: Some(runtime),
        }
    }

    /// Wakes the writers on the thread that calls [`Self::wake_all`].
    #[must_use]
    pub fn on_this_thread() -> Self {
        Self {
            signals: Vec::new(),
            runtime: None,
        }
    }

    pub(crate) fn push(&mut self, signal: &Arc<WriterSignal>) {
        self.signals.push(Arc::clone(signal));
    }

    /// Wakes every writer that a connection pushed since the last call.
    pub fn wake_all(&mut self) {
        if self.signals.is_empty() {
            return;
        }
        let signals = mem::take(&mut self.signals);
        match &self.runtime {
            Some(runtime) => {
                runtime.spawn(async move {
                    for signal in signals {
                        signal.wake();
                    }
                });
            }
            None => {
                for signal in signals {
                    signal.wake();
                }
            }
        }
    }
}

impl Drop for WriterWakes {
    fn drop(&mut self) {
        self.wake_all();
    }
}
