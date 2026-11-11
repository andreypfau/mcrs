#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

mod common;
mod components;
mod hashed_stack;
mod item_stack;
mod saved;
mod wire;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
