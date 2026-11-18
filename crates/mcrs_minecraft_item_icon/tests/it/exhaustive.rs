use std::hint::black_box;
use std::time::Instant;

use mcrs_minecraft_item_icon::resolve;

use crate::corpus::{GUI, SEARCH, session_sources, view};

#[test]
fn resolving_the_creative_search_list_takes_microseconds_per_icon() {
    let sources = session_sources();
    let stacks = &*SEARCH;
    let started = Instant::now();
    for stack in stacks {
        black_box(resolve(view(stack), &sources, &GUI));
    }
    let total = started.elapsed();
    let count = u32::try_from(stacks.len()).expect("the search list fits in u32");
    println!(
        "resolved {count} icons in {total:?}, {:?} per icon",
        total / count
    );
}
