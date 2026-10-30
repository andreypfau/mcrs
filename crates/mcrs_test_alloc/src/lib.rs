//! A global allocator that counts the allocations of the calling thread, for tests.
// std: `System` is the allocator that it forwards to.

mod counting_allocator;

pub use counting_allocator::{CountingAllocator, allocations};
