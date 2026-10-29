use crate::{OP, read};
use mcrs_minecraft_command_coordinates::{BlockPosArgument, Vec3Argument};
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
fn a_warm_read_of_the_coordinates_allocates_nothing() {
    let coordinates = ["~1 ^ 3", "^ ^1 ^-2.5", "1 2 3"];
    assert!(read(&Vec3Argument, coordinates[0], &OP).0.is_err());
    assert!(read(&Vec3Argument, coordinates[1], &OP).0.is_ok());
    assert!(read(&BlockPosArgument, coordinates[2], &OP).0.is_ok());

    let run = || {
        for input in coordinates {
            let _ = std::hint::black_box(read(&Vec3Argument, input, &OP));
            let _ = std::hint::black_box(read(&BlockPosArgument, input, &OP));
        }
    };
    run();
    let before = ALLOCATIONS.with(Cell::get);
    run();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 0);
}
