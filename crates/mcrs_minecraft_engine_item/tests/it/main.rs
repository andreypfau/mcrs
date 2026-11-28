#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod allocations;
mod ambiguity;
mod effective_state;
mod equipped;
mod fixture;
mod placement;
mod request;
mod request_check;
mod round_trip;
mod slot_changed;
mod slot_policy;
mod usage;
mod write_in_place;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
