use std::cell::Cell;

use crate::{ErrorKind, Result};

/// How deep [`nested`] lets a value contain values of its own kind.
pub const MAX_NESTING: u32 = 64;

thread_local! {
    static NESTING: Cell<u32> = const { Cell::new(0) };
}

/// Bounds the recursion of a self-containing wire value, since the layout
/// alone lets a few kilobytes of wrappers overflow the stack. A Java client
/// only loses the connection to that; a Rust one would abort.
pub fn nested<T>(decode: impl FnOnce() -> Result<T>) -> Result<T> {
    struct Unwind(u32);
    impl Drop for Unwind {
        fn drop(&mut self) {
            NESTING.set(self.0);
        }
    }
    let depth = NESTING.get();
    if depth >= MAX_NESTING {
        return Err(ErrorKind::TooDeep { max: MAX_NESTING }.into());
    }
    NESTING.set(depth + 1);
    let _unwind = Unwind(depth);
    decode()
}
