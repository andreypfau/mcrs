thread_local! {
    static BINARY_READS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[derive(Debug)]
pub(crate) struct BinaryReadGuard;

impl BinaryReadGuard {
    pub(crate) fn new() -> Self {
        BINARY_READS.with(|reads| reads.set(reads.get() + 1));
        BinaryReadGuard
    }
}

impl Drop for BinaryReadGuard {
    fn drop(&mut self) {
        BINARY_READS.with(|reads| reads.set(reads.get() - 1));
    }
}

pub(crate) fn reading_binary() -> bool {
    BINARY_READS.with(|reads| reads.get() > 0)
}
