#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod allocations;
mod ambiguity;
mod effective_state;
mod fixture;
mod round_trip;
mod usage;
mod write_in_place;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
