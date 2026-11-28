use std::cell::Cell;

use crate::{ErrorKind, Result};

pub const MAX_NESTING: u32 = 64;

thread_local! {
    static NESTING: Cell<u32> = const { Cell::new(0) };
}

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
