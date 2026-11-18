use std::hint::black_box;

use mcrs_minecraft_item_icon::resolve;
use mcrs_test_alloc::{CountingAllocator, allocations};

use crate::corpus::{GUI, SEARCH, session_sources, view};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn resolving_the_creative_search_list_allocates_nothing() {
    let sources = session_sources();
    let stacks = &*SEARCH;
    assert!(
        stacks.len() > 1000,
        "the search list holds the vanilla items"
    );
    let before = allocations();
    for stack in stacks {
        black_box(resolve(view(stack), &sources, &GUI));
    }
    assert_eq!(allocations() - before, 0);
}
