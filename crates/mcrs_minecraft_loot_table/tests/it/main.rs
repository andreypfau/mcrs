#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

mod block_loot;
mod entry_types;
mod fixture;
mod item_modifiers;
mod loot_tables;
mod replay;

use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;
