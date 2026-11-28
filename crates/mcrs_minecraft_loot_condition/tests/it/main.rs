#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use mcrs_test_alloc::CountingAllocator;

mod allocation;
mod condition;
mod host;
mod number_provider;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
