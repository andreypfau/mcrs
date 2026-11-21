use core::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUILD: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RouterKey {
    build: u64,
    seed: u64,
}

impl RouterKey {
    pub(crate) fn new(build: u64, seed: u64) -> Self {
        Self { build, seed }
    }
}

pub(crate) fn next_build() -> u64 {
    NEXT_BUILD.fetch_add(1, Ordering::Relaxed)
}
