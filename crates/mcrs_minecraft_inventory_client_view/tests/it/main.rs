#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

mod client_view;
mod state_id;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
