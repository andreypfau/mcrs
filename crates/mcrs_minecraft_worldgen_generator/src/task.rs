use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Token for cooperative cancellation of chunk generation tasks.
///
/// The token is cloned and passed to worker tasks. When `cancel()` is called,
/// tasks check `is_cancelled()` between section generations and can exit early.
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Create a new uncancelled token.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Signal cancellation to all clones of this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Check if cancellation has been signaled.
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
