use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

/// The allocations that the calling thread made since it started. A `realloc` and an
/// `alloc_zeroed` count as one allocation each.
#[must_use]
pub fn allocations() -> usize {
    ALLOCATIONS.try_with(Cell::get).unwrap_or(0)
}

fn count() {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get().wrapping_add(1)));
}

/// Forwards to `System` and counts every request for memory. Declare it as the
/// `#[global_allocator]` of a test binary.
pub struct CountingAllocator;

// SAFETY: every call forwards to `System` with the arguments it got, so `System` upholds the
// contract of `GlobalAlloc`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds the contract of `alloc` for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds the contract of `alloc_zeroed` for `layout`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` through this allocator with `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: `ptr` came from `System` through this allocator with `layout`, and the caller
        // upholds the contract of `realloc` for `size`.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn an_allocation_is_counted() {
        let before = allocations();
        let boxed = std::hint::black_box(Box::new(7_u64));
        assert!(allocations() > before, "the counter did not see the box");
        drop(boxed);
    }
}
