use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

/// What a connection and its writer task tell each other besides the batches: that batches
/// wait, that the connection is gone, and that the socket died.
///
/// The writer never waits on the channel of the batches, so a send into it wakes nobody: the
/// connection decides when the writer wakes.
#[derive(Debug, Default)]
pub(crate) struct WriterSignal {
    wake: Notify,
    closed: AtomicBool,
    disconnected: AtomicBool,
}

impl WriterSignal {
    /// A wake that finds the writer busy is kept, so the writer checks the channel once more
    /// before it waits.
    pub(crate) fn wake(&self) {
        self.wake.notify_one();
    }

    pub(crate) async fn woken(&self) {
        self.wake.notified().await;
    }

    /// The connection sends nothing more. Everything it sent before is in the channel when the
    /// writer reads this.
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake();
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn mark_disconnected(&self) {
        self.disconnected.store(true, Ordering::Relaxed);
    }

    pub(crate) fn disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Relaxed)
    }
}
