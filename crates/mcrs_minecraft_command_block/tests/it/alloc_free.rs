use crate::block::{read, suggest_block};
use mcrs_command::Suggestion;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn a_warm_read_and_suggestion_of_a_block_predicate_allocates_nothing() {
    let blocks = [
        "chest{Items:[{id:\"minecraft:stone\",Slot:0b,count:1}],x:-12,a:[I;1,2],b:1.5e3f,c:'\\N{SNOWMAN}\\uD83D\\uDE00',d:uuid(\"00000001-0002-0003-0004-000000000005\"),e:bool(1)}",
        "#logs[axis=y]{a:[B;300]}",
        "stone{a:1",
        "oak_stairs[facing=nor",
        "#logs[axis=",
    ];
    assert_eq!(read(blocks[0]).0.map(drop), Ok(()));
    assert_eq!(read(blocks[1]).0.map_err(|e| e.position), Err(23));
    assert_eq!(read(blocks[2]).0.map_err(|e| e.position), Err(9));

    let mut out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut suggestions = 0;
    let mut run = || {
        suggestions = 0;
        for input in blocks {
            let (value, cursor) = read(input);
            std::hint::black_box((value.is_ok(), cursor));
            out.clear();
            suggest_block(input, &mut out);
            suggestions += out.len();
        }
    };
    run();
    let before = ALLOCATIONS.with(Cell::get);
    run();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 0);
    assert!(suggestions > 0);
}
