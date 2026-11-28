use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(crate) struct WriterSignal {
    wake: Notify,
    closed: AtomicBool,
    disconnected: AtomicBool,
}

impl WriterSignal {
    pub(crate) fn wake(&self) {
        self.wake.notify_one();
    }

    pub(crate) async fn woken(&self) {
        self.wake.notified().await;
    }

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
