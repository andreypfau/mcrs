#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

mod allocations;
mod capture;
mod click;
mod close;
mod creative;
mod fixture;
mod full_state;
mod hand;
mod player_file;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
