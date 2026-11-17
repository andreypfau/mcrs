use std::mem;
use std::sync::Arc;

use tokio::runtime::Handle;

use crate::writer_signal::WriterSignal;

pub struct WriterWakes {
    signals: Vec<Arc<WriterSignal>>,
    runtime: Option<Handle>,
}

impl WriterWakes {
    #[must_use]
    pub fn on_runtime(runtime: Handle) -> Self {
        Self {
            signals: Vec::new(),
            runtime: Some(runtime),
        }
    }

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
