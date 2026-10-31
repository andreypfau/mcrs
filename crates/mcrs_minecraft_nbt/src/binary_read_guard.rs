thread_local! {
    static BINARY_READS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// serde's buffering deserializers (`flatten`, `tag`, `untagged`) replay a
/// value through one that reports itself human-readable, which would make a
/// tag read from NBT below them look JSON-sourced and get its numbers
/// narrowed. A live NBT deserializer on the thread settles the question.
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

// This NBT crate is inspired from CrabNBT
