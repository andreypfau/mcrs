use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ColumnSource {
    Saved,
    Generated,
}

impl ColumnSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Saved => "save",
            Self::Generated => "worldgen",
        }
    }
}
